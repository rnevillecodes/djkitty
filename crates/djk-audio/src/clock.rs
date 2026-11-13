//! Output timing: when each rendered block is heard, and the output latency to report.
//!
//! Every audio callback measures its own output latency, and both that measurement and the
//! moment the callback runs jitter by a few milliseconds. The two errors are linked: a
//! callback that runs late finds less audio buffered ahead of it, so it also measures a
//! shorter latency. Averaging the latency on its own would break that link and let the
//! callback's lateness through to the playhead.
//!
//! So the clock tracks the one thing that is steady: the instant each rendered frame is
//! heard, which advances at exactly the device rate. Each callback's measurement nudges
//! that line a little, and the block's published timestamp is taken from the line, so
//! `timestamp + latency` (what the UI extrapolates the playhead from) stays smooth
//! whichever of the two measurements is noisy.
//!
//! Runs on the audio thread: no allocation, locking or I/O.

use std::time::{Duration, Instant};

/// Time constant of the averages, in seconds of audio. Each callback is weighted by the
/// audio it carries, so the response is the same at every buffer size.
const SMOOTHING_SECS: f64 = 0.5;
/// A measurement this far (seconds) from the tracked timing is a real change, such as a
/// dropout or a device reconfiguring itself, and restarts the averages.
const RESYNC_SECS: f64 = 0.05;

/// Tracks when rendered audio is heard. See the module docs.
#[derive(Debug, Clone)]
pub struct OutputClock {
    rate: f64,
    /// The first callback's instant; other times are seconds from here.
    origin: Option<Instant>,
    /// Frames rendered up to the end of the latest block.
    frames: u64,
    /// Averaged instant (seconds from `origin`) at which frame 0 is heard.
    heard_start: f64,
    /// Averaged output latency in seconds.
    latency: f64,
    /// Measurements in the current averages.
    samples: u32,
}

impl OutputClock {
    pub fn new(device_rate: u32) -> Self {
        Self {
            rate: f64::from(device_rate.max(1)),
            origin: None,
            frames: 0,
            heard_start: 0.0,
            latency: 0.0,
            samples: 0,
        }
    }

    /// The averaged output latency in seconds (0 until the first measurement).
    pub fn latency(&self) -> f64 {
        self.latency
    }

    /// Record one callback: it ran at `now`, renders `block_frames` frames, and the end
    /// of its block will be heard `latency_secs` after `now`. Returns the timestamp to
    /// publish for the block, which is `now` with the callback's own jitter taken out.
    pub fn update(&mut self, now: Instant, latency_secs: f64, block_frames: usize) -> Instant {
        self.frames += block_frames as u64;
        let origin = *self.origin.get_or_insert(now);
        let rendered = self.frames as f64 / self.rate;
        if latency_secs.is_finite() && latency_secs >= 0.0 {
            let now_secs = now.saturating_duration_since(origin).as_secs_f64();
            let heard_start = now_secs + latency_secs - rendered;
            if self.samples > 0 && (heard_start - self.heard_start).abs() > RESYNC_SECS {
                self.samples = 0;
            }
            self.samples = self.samples.saturating_add(1);
            // A plain mean over the first measurements, so one odd reading at start-up is
            // averaged away quickly, then an exponential average.
            let block_secs = block_frames as f64 / self.rate;
            let weight = (1.0 - (-block_secs / SMOOTHING_SECS).exp())
                .max(1.0 / f64::from(self.samples))
                .min(1.0);
            self.heard_start += (heard_start - self.heard_start) * weight;
            self.latency += (latency_secs - self.latency) * weight;
        }
        if self.samples == 0 {
            return now;
        }
        // The block ends at `heard_start + rendered`, heard `latency` after its timestamp.
        let secs = self.heard_start + rendered - self.latency;
        if secs >= 0.0 {
            origin + Duration::from_secs_f64(secs)
        } else {
            origin
                .checked_sub(Duration::from_secs_f64(-secs))
                .unwrap_or(now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    const BLOCK: usize = 480;
    const BLOCK_SECS: f64 = BLOCK as f64 / RATE as f64;

    /// Seconds between `a` and `b`, signed.
    fn diff(a: Instant, b: Instant) -> f64 {
        if a >= b {
            (a - b).as_secs_f64()
        } else {
            -(b - a).as_secs_f64()
        }
    }

    /// Runs `n` callbacks. Callback `i` is due at `i` blocks, runs `late(i)` seconds
    /// after that and measures `noise(i)` seconds of error on top of the true latency.
    /// Returns the largest error (seconds) in when the UI would think a block is heard,
    /// and the final reported latency.
    fn run(
        n: usize,
        true_latency: f64,
        late: impl Fn(usize) -> f64,
        noise: impl Fn(usize) -> f64,
    ) -> (f64, f64) {
        let start = Instant::now();
        let mut clock = OutputClock::new(RATE);
        let mut worst: f64 = 0.0;
        for i in 0..n {
            let due = i as f64 * BLOCK_SECS;
            let now = start + Duration::from_secs_f64(due + late(i));
            // A late callback finds that much less audio buffered ahead of it.
            let measured = true_latency - late(i) + noise(i);
            let ts = clock.update(now, measured, BLOCK);
            let heard = diff(ts, start) + clock.latency();
            // Let the averages settle before judging.
            if i >= 200 {
                worst = worst.max((heard - (due + true_latency)).abs());
            }
        }
        (worst, clock.latency())
    }

    #[test]
    fn late_callbacks_do_not_move_the_playhead() {
        // Callbacks alternately on time and 3 ms late.
        let (worst, latency) = run(
            600,
            0.020,
            |i| if i % 2 == 0 { 0.0 } else { 0.003 },
            |_| 0.0,
        );
        assert!(worst < 1e-6, "heard-time error {worst}");
        assert!((latency - 0.0185).abs() < 0.0005, "latency {latency}");
    }

    #[test]
    fn noisy_latency_measurements_are_averaged() {
        let (worst, latency) = run(
            600,
            0.020,
            |_| 0.0,
            |i| if i % 2 == 0 { 0.003 } else { -0.003 },
        );
        assert!(worst < 0.0002, "heard-time error {worst}");
        assert!((latency - 0.020).abs() < 0.0002, "latency {latency}");
    }

    #[test]
    fn first_measurement_is_averaged_away_quickly() {
        let start = Instant::now();
        let mut clock = OutputClock::new(RATE);
        for i in 0..20 {
            let now = start + Duration::from_secs_f64(i as f64 * BLOCK_SECS);
            // The first callback reports 40 ms; the device really has 10 ms.
            clock.update(now, if i == 0 { 0.040 } else { 0.010 }, BLOCK);
        }
        // A fixed 2% weight per callback would still be near 30 ms here.
        assert!(
            (clock.latency() - 0.010).abs() < 0.002,
            "{}",
            clock.latency()
        );
    }

    #[test]
    fn settles_equally_fast_at_any_buffer_size() {
        // A second after a 10 ms step, small and large buffers have moved alike.
        let settle = |block: usize| {
            let start = Instant::now();
            let mut clock = OutputClock::new(RATE);
            let per_sec = RATE as usize / block;
            for i in 0..per_sec * 5 {
                let now = start + Duration::from_secs_f64((i * block) as f64 / f64::from(RATE));
                let latency = if i < per_sec * 4 { 0.020 } else { 0.030 };
                clock.update(now, latency, block);
            }
            clock.latency()
        };
        let (small, large) = (settle(64), settle(1920));
        assert!((small - large).abs() < 0.0005, "{small} vs {large}");
        assert!(small > 0.027 && small < 0.030, "{small}");
    }

    #[test]
    fn a_large_jump_restarts_the_averages() {
        let start = Instant::now();
        let mut clock = OutputClock::new(RATE);
        let mut now = start;
        for _ in 0..300 {
            clock.update(now, 0.010, BLOCK);
            now += Duration::from_secs_f64(BLOCK_SECS);
        }
        // A dropout: 200 ms passes without a callback.
        now += Duration::from_millis(200);
        let ts = clock.update(now, 0.010, BLOCK);
        assert!(diff(ts, now).abs() < 1e-6);
        assert!((clock.latency() - 0.010).abs() < 1e-9);
    }

    #[test]
    fn junk_measurements_are_ignored() {
        let now = Instant::now();
        let mut clock = OutputClock::new(RATE);
        assert_eq!(clock.update(now, f64::NAN, BLOCK), now);
        assert_eq!(clock.latency(), 0.0);
        clock.update(now, 0.010, BLOCK);
        clock.update(now, -1.0, BLOCK);
        clock.update(now, f64::INFINITY, BLOCK);
        assert_eq!(clock.latency(), 0.010);
    }
}
