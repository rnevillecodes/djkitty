//! Per-channel 3-band isolator EQ.
//!
//! The signal is split into low, mid and high bands with 4th-order Linkwitz-Riley
//! crossovers, each band is scaled by its knob, and the bands are summed. Linkwitz-Riley
//! bands add back to a flat magnitude response, so with every knob at centre the EQ is
//! transparent (it only shifts phase, inaudibly). Turning a knob fully left removes its
//! band completely ("kill"), which is what DJs use for bass swaps.
//!
//! Runs on the audio thread: no allocation, locking or I/O. Gain changes are smoothed
//! per sample so knob moves never click.

use djk_core::EQ_NEUTRAL;

/// Low/mid crossover in Hz.
pub const LOW_MID_HZ: f32 = 250.0;
/// Mid/high crossover in Hz.
pub const MID_HIGH_HZ: f32 = 4_000.0;
/// Boost at the top of a knob's travel, in dB.
pub const MAX_BOOST_DB: f32 = 6.0;
/// Cut just above the kill zone, in dB. Most of the lower half of the knob covers
/// −26..0 dB; the last few percent fade from there to silence.
pub const DEEP_CUT_DB: f32 = -26.0;
/// Fraction of the lower half of the knob (from the far left) that fades to a full kill.
const KILL_ZONE: f32 = 0.06;
/// Time constant for gain smoothing, in seconds.
const SMOOTHING_SECS: f32 = 0.01;

/// Linear gain for an EQ knob position (0 kill, 0.5 = 0 dB, 1 = +6 dB).
pub fn knob_gain(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= EQ_NEUTRAL {
        let db = MAX_BOOST_DB * (t - EQ_NEUTRAL) / (1.0 - EQ_NEUTRAL);
        return db_to_gain(db);
    }
    let u = t / EQ_NEUTRAL; // 0 (far left) .. 1 (centre)
    if u >= KILL_ZONE {
        let db = DEEP_CUT_DB * (1.0 - u) / (1.0 - KILL_ZONE);
        db_to_gain(db)
    } else {
        db_to_gain(DEEP_CUT_DB) * (u / KILL_ZONE)
    }
}

/// The knob's gain in dB for display (`None` for a full kill).
pub fn knob_db(t: f32) -> Option<f32> {
    let g = knob_gain(t);
    (g > 0.0).then(|| 20.0 * g.log10())
}

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// A transposed direct form II biquad.
#[derive(Clone, Copy, Debug, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// 2nd-order Butterworth low- or high-pass (RBJ cookbook, Q = 1/√2).
    fn butterworth(rate: f32, hz: f32, high: bool) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * (hz / rate).min(0.49);
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        let (b0, b1, b2) = if high {
            ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0)
        } else {
            ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0)
        };
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// 4th-order Linkwitz-Riley filter: two identical Butterworth sections.
#[derive(Clone, Copy, Debug, Default)]
struct Lr4([Biquad; 2]);

impl Lr4 {
    fn new(rate: f32, hz: f32, high: bool) -> Self {
        let b = Biquad::butterworth(rate, hz, high);
        Self([b, b])
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.0[0].process(x);
        self.0[1].process(y)
    }
}

/// The crossover network for one audio channel.
#[derive(Clone, Copy, Debug, Default)]
struct Splitter {
    low: Lr4,
    rest: Lr4,
    mid: Lr4,
    high: Lr4,
    /// The low band also passes through the second crossover (split and summed), so its
    /// phase matches the mid and high bands and the three sum flat.
    low_align_lp: Lr4,
    low_align_hp: Lr4,
}

impl Splitter {
    fn new(rate: f32) -> Self {
        Self {
            low: Lr4::new(rate, LOW_MID_HZ, false),
            rest: Lr4::new(rate, LOW_MID_HZ, true),
            mid: Lr4::new(rate, MID_HIGH_HZ, false),
            high: Lr4::new(rate, MID_HIGH_HZ, true),
            low_align_lp: Lr4::new(rate, MID_HIGH_HZ, false),
            low_align_hp: Lr4::new(rate, MID_HIGH_HZ, true),
        }
    }

    fn is_silent(&self) -> bool {
        [
            &self.low,
            &self.rest,
            &self.mid,
            &self.high,
            &self.low_align_lp,
            &self.low_align_hp,
        ]
        .iter()
        .all(|f| f.0.iter().all(|b| b.z1 == 0.0 && b.z2 == 0.0))
    }

    /// Split one sample into `[low, mid, high]`.
    #[inline]
    fn split(&mut self, x: f32) -> [f32; 3] {
        let low = self.low.process(x);
        let low = self.low_align_lp.process(low) + self.low_align_hp.process(low);
        let rest = self.rest.process(x);
        [low, self.mid.process(rest), self.high.process(rest)]
    }
}

/// A stereo 3-band EQ for one channel.
#[derive(Clone, Debug)]
pub struct Eq {
    left: Splitter,
    right: Splitter,
    /// Current (smoothed) and target linear gains, `[low, mid, high]`.
    gains: [f32; 3],
    targets: [f32; 3],
    smoothing: f32,
}

impl Eq {
    pub fn new(sample_rate: u32) -> Self {
        let mut eq = Self {
            left: Splitter::default(),
            right: Splitter::default(),
            gains: [1.0; 3],
            targets: [1.0; 3],
            smoothing: 0.0,
        };
        eq.set_sample_rate(sample_rate);
        eq
    }

    /// Rebuild the filters for a new rate (resets their state). Not real-time; call while
    /// no stream is running.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        let rate = sample_rate.max(1) as f32;
        self.left = Splitter::new(rate);
        self.right = Splitter::new(rate);
        self.smoothing = 1.0 - (-1.0 / (SMOOTHING_SECS * rate)).exp();
    }

    /// Set the knob positions `[low, mid, high]` (0 kill, 0.5 = 0 dB, 1 = +6 dB). The
    /// gains glide to them over ~10 ms.
    pub fn set_knobs(&mut self, knobs: [f32; 3]) {
        self.targets = knobs.map(knob_gain);
    }

    /// True when no sound is left in the filters, so silence in gives silence out.
    pub fn is_silent(&self) -> bool {
        self.left.is_silent() && self.right.is_silent()
    }

    /// Jump the gains to the knob positions. For blocks skipped while silent, where
    /// there is nothing to click.
    pub fn settle(&mut self) {
        self.gains = self.targets;
    }

    /// Process a block of stereo audio in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            for (g, t) in self.gains.iter_mut().zip(self.targets) {
                *g += (t - *g) * self.smoothing;
            }
            let bl = self.left.split(*l);
            let br = self.right.split(*r);
            *l = bl[0] * self.gains[0] + bl[1] * self.gains[1] + bl[2] * self.gains[2];
            *r = br[0] * self.gains[0] + br[1] * self.gains[1] + br[2] * self.gains[2];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// Steady-state gain (in dB) of the EQ for a sine at `hz`.
    fn response_db(knobs: [f32; 3], hz: f32) -> f32 {
        let mut eq = Eq::new(RATE);
        eq.set_knobs(knobs);
        eq.gains = eq.targets; // skip the glide
        let n = RATE as usize; // 1 s: settle for half, measure the second half
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / RATE as f32).sin() * 0.5)
            .collect();
        let mut l = input.clone();
        let mut r = input.clone();
        eq.process(&mut l, &mut r);
        let rms = |s: &[f32]| (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt();
        20.0 * (rms(&l[n / 2..]) / rms(&input[n / 2..])).log10()
    }

    const CENTRE: [f32; 3] = [EQ_NEUTRAL; 3];

    #[test]
    fn knob_curve() {
        assert!((knob_gain(EQ_NEUTRAL) - 1.0).abs() < 1e-6);
        assert!((knob_db(1.0).unwrap() - 6.0).abs() < 1e-3);
        assert_eq!(knob_gain(0.0), 0.0);
        assert!(knob_db(0.0).is_none());
        // A quarter of the way round (half of the cut range) is a big but not total cut.
        let quarter = knob_db(0.25).unwrap();
        assert!((-15.0..-10.0).contains(&quarter), "{quarter}");
        // Gain never decreases as the knob turns right.
        let mut last = 0.0;
        for i in 0..=100 {
            let g = knob_gain(i as f32 / 100.0);
            assert!(g >= last);
            last = g;
        }
    }

    #[test]
    fn centred_knobs_are_flat() {
        for hz in [50.0, 250.0, 1_000.0, 4_000.0, 10_000.0] {
            let db = response_db(CENTRE, hz);
            assert!(db.abs() < 0.1, "{hz} Hz: {db} dB");
        }
    }

    #[test]
    fn each_kill_removes_only_its_band() {
        let cases = [
            (0, 50.0, [1_000.0, 10_000.0]),
            (1, 1_000.0, [50.0, 12_000.0]),
            (2, 12_000.0, [50.0, 1_000.0]),
        ];
        for (band, killed_hz, kept) in cases {
            let mut knobs = CENTRE;
            knobs[band] = 0.0;
            let cut = response_db(knobs, killed_hz);
            assert!(cut < -40.0, "band {band} at {killed_hz} Hz: {cut} dB");
            for hz in kept {
                let db = response_db(knobs, hz);
                assert!(db.abs() < 1.0, "band {band} kill, {hz} Hz: {db} dB");
            }
        }
    }

    #[test]
    fn full_boost_adds_six_db() {
        let db = response_db([1.0, EQ_NEUTRAL, EQ_NEUTRAL], 50.0);
        assert!((db - 6.0).abs() < 0.3, "{db}");
    }

    #[test]
    fn snapping_a_knob_does_not_click() {
        // A steady low tone; kill the low band instantly and check the output never jumps
        // by more than the input could on its own.
        let mut eq = Eq::new(RATE);
        let n = RATE as usize / 2;
        let hz = 60.0;
        let mut l: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / RATE as f32).sin() * 0.8)
            .collect();
        let mut r = l.clone();
        let half = n / 2;
        eq.process(&mut l[..half], &mut r[..half]);
        eq.set_knobs([0.0, EQ_NEUTRAL, EQ_NEUTRAL]);
        eq.process(&mut l[half..], &mut r[half..]);
        let max_step = l
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        // A 60 Hz sine at 0.8 moves at most 2π·60·0.8/48000 ≈ 0.0063 per sample.
        assert!(max_step < 0.01, "step {max_step}");
        // ...and the low band does go away.
        let tail = &l[n - 2000..];
        let peak = tail.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak < 0.01, "residual {peak}");
    }

    #[test]
    fn settle_jumps_to_the_knob_gains() {
        let mut eq = Eq::new(48_000);
        eq.set_knobs([0.0, 0.5, 0.5]);
        eq.settle();
        assert_eq!(eq.gains, eq.targets);
        assert_eq!(eq.gains[0], 0.0);
    }

    /// With denormals flushed (as on the audio thread), an EQ fed silence rings out to
    /// exactly zero, so the engine can stop running it.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn rings_out_to_silence() {
        let _guard = crate::denormal::NoDenormals::enter();
        let mut eq = Eq::new(48_000);
        assert!(eq.is_silent());
        let mut l: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let mut r = l.clone();
        eq.process(&mut l, &mut r);
        assert!(!eq.is_silent());
        let mut blocks = 0;
        while !eq.is_silent() {
            let (mut l, mut r) = ([0.0f32; 1024], [0.0f32; 1024]);
            eq.process(&mut l, &mut r);
            blocks += 1;
            assert!(blocks < 100, "EQ never rang out");
        }
    }
}
