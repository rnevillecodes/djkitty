//! Per-channel bipolar DJ filter (the "CFX" knob filter).
//!
//! One knob per deck: left of centre sweeps a low-pass filter down, right of centre sweeps
//! a high-pass filter up, and around the centre the filter is fully bypassed. It is a
//! 2nd-order (12 dB/oct) Cytomic/Simper trapezoidal state-variable filter, which stays
//! stable while its cutoff moves quickly and gives both outputs from one structure.
//!
//! The cutoff glides in the log-frequency domain and the filtered signal is cross-faded
//! with the dry one, so knob moves never click. Switching between low- and high-pass (or
//! to off) always fades the filter out first; once fully faded it is bypassed and the
//! output is bit-identical to the input.
//!
//! Runs on the audio thread: no allocation, locking or I/O.

use djk_core::FILTER_NEUTRAL;

/// Distance from the centre (in knob units) inside which the filter is off. Just wider
/// than the on-screen knob's centre snap.
pub const DEAD_ZONE: f32 = 0.02;
/// Low-pass cutoff just outside the dead zone, in Hz.
pub const LPF_OPEN_HZ: f32 = 20_000.0;
/// Low-pass cutoff at the far left, in Hz.
pub const LPF_CLOSED_HZ: f32 = 100.0;
/// High-pass cutoff just outside the dead zone, in Hz.
pub const HPF_OPEN_HZ: f32 = 20.0;
/// High-pass cutoff at the far right, in Hz.
pub const HPF_CLOSED_HZ: f32 = 8_000.0;
/// Resonance: about +0.7 dB at the cutoff, a gentle DJ-filter peak without ringing.
const Q: f32 = 0.9;
/// Highest cutoff as a fraction of the sample rate, so the filter stays well-behaved.
const MAX_CUTOFF_FRACTION: f32 = 0.45;
/// Time constant for cutoff and wet-mix smoothing, in seconds.
const SMOOTHING_SECS: f32 = 0.01;
/// Samples between coefficient updates while the cutoff is gliding.
const COEFF_INTERVAL: u32 = 16;
/// Wet mix below which a fading-out filter counts as silent and is bypassed.
const WET_FLOOR: f32 = 1e-4;

/// What a knob position asks for.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FilterMode {
    Off,
    LowPass(f32),
    HighPass(f32),
}

/// The filter type and cutoff (Hz) for a knob position (0 full low-pass, 0.5 off,
/// 1 full high-pass). The cutoff is spaced logarithmically along the travel.
pub fn mode(value: f32) -> FilterMode {
    let value = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        FILTER_NEUTRAL
    };
    let d = value - FILTER_NEUTRAL;
    if d.abs() <= DEAD_ZONE {
        return FilterMode::Off;
    }
    let u = ((d.abs() - DEAD_ZONE) / (0.5 - DEAD_ZONE)).clamp(0.0, 1.0);
    if d < 0.0 {
        FilterMode::LowPass(LPF_OPEN_HZ * (LPF_CLOSED_HZ / LPF_OPEN_HZ).powf(u))
    } else {
        FilterMode::HighPass(HPF_OPEN_HZ * (HPF_CLOSED_HZ / HPF_OPEN_HZ).powf(u))
    }
}

/// Whether a knob position changes the sound.
pub fn is_active(value: f32) -> bool {
    mode(value) != FilterMode::Off
}

/// Human-readable filter state for a knob position, e.g. "LPF 800 Hz", "HPF 2.1 kHz" or
/// "off". Allocates; for the UI only.
pub fn describe(value: f32) -> String {
    fn hz(f: f32) -> String {
        if f >= 10_000.0 {
            format!("{:.0} kHz", f / 1000.0)
        } else if f >= 1000.0 {
            format!("{:.1} kHz", f / 1000.0)
        } else {
            format!("{:.0} Hz", f)
        }
    }
    match mode(value) {
        FilterMode::Off => "off".to_owned(),
        FilterMode::LowPass(f) => format!("LPF {}", hz(f)),
        FilterMode::HighPass(f) => format!("HPF {}", hz(f)),
    }
}

/// Integrator state of one SVF channel.
#[derive(Copy, Clone, Debug, Default)]
struct SvfState {
    ic1: f32,
    ic2: f32,
}

/// Coefficients shared by both channels.
#[derive(Copy, Clone, Debug, Default)]
struct SvfCoeffs {
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl SvfCoeffs {
    fn new(rate: f32, hz: f32) -> Self {
        let g = (std::f32::consts::PI * hz / rate).tan();
        let k = 1.0 / Q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        Self {
            a1,
            a2,
            a3: g * a2,
            k,
        }
    }
}

impl SvfState {
    /// One sample through the SVF; returns the low-pass or high-pass output.
    #[inline]
    fn tick(&mut self, c: &SvfCoeffs, x: f32, high: bool) -> f32 {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        if high { x - c.k * v1 - v2 } else { v2 }
    }
}

/// A stereo bipolar filter for one channel.
#[derive(Clone, Debug)]
pub struct Filter {
    rate: f32,
    smoothing: f32,
    /// What the knob currently asks for.
    target: FilterMode,
    /// Running mode while not bypassed: high-pass if true, else low-pass.
    high: bool,
    /// Bypassed: wet mix is 0 and the filter state is reset; output equals input.
    bypassed: bool,
    /// Smoothed and target cutoff, as ln(Hz).
    log_hz: f32,
    target_log_hz: f32,
    /// Smoothed wet mix (0 dry, 1 filtered).
    wet: f32,
    coeffs: SvfCoeffs,
    countdown: u32,
    state: [SvfState; 2],
}

impl Filter {
    pub fn new(sample_rate: u32) -> Self {
        let mut f = Self {
            rate: 48_000.0,
            smoothing: 0.0,
            target: FilterMode::Off,
            high: false,
            bypassed: true,
            log_hz: LPF_OPEN_HZ.ln(),
            target_log_hz: LPF_OPEN_HZ.ln(),
            wet: 0.0,
            coeffs: SvfCoeffs::default(),
            countdown: 0,
            state: [SvfState::default(); 2],
        };
        f.set_sample_rate(sample_rate);
        f
    }

    /// Change the sample rate (resets to bypassed). Not real-time; call while no stream
    /// is running.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.rate = sample_rate.max(1) as f32;
        self.smoothing = 1.0 - (-1.0 / (SMOOTHING_SECS * self.rate)).exp();
        self.bypass();
    }

    /// Set the knob position (0 full low-pass, 0.5 off, 1 full high-pass). The filter
    /// glides to it over ~10 ms.
    pub fn set_position(&mut self, value: f32) {
        self.target = mode(value);
    }

    fn bypass(&mut self) {
        self.bypassed = true;
        self.wet = 0.0;
        self.state = [SvfState::default(); 2];
        self.countdown = 0;
    }

    fn clamp_hz(&self, hz: f32) -> f32 {
        hz.min(MAX_CUTOFF_FRACTION * self.rate).max(10.0)
    }

    /// Process a block of stereo audio in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.bypassed {
            let (high, hz) = match self.target {
                FilterMode::Off => return, // nothing to do: output is the input
                FilterMode::LowPass(hz) => (false, hz),
                FilterMode::HighPass(hz) => (true, hz),
            };
            // Wake up from the open end of the sweep, so the filter fades in from (almost)
            // no effect and glides to the requested cutoff.
            self.bypassed = false;
            self.high = high;
            let open = if high { HPF_OPEN_HZ } else { LPF_OPEN_HZ };
            self.log_hz = self.clamp_hz(open).ln();
            self.target_log_hz = self.clamp_hz(hz).ln();
            self.countdown = 0;
        }

        // Where the wet mix is heading: 1 while the target matches the running mode,
        // otherwise fade out first (to off, or before switching low-pass <-> high-pass).
        let wet_target = match self.target {
            FilterMode::LowPass(hz) if !self.high => {
                self.target_log_hz = self.clamp_hz(hz).ln();
                1.0
            }
            FilterMode::HighPass(hz) if self.high => {
                self.target_log_hz = self.clamp_hz(hz).ln();
                1.0
            }
            _ => 0.0,
        };

        let s = self.smoothing;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.wet += (wet_target - self.wet) * s;
            self.log_hz += (self.target_log_hz - self.log_hz) * s;
            if self.countdown == 0 {
                self.coeffs = SvfCoeffs::new(self.rate, self.log_hz.exp());
                self.countdown = COEFF_INTERVAL;
            }
            self.countdown -= 1;

            let fl = self.state[0].tick(&self.coeffs, *l, self.high);
            let fr = self.state[1].tick(&self.coeffs, *r, self.high);
            *l += (fl - *l) * self.wet;
            *r += (fr - *r) * self.wet;
        }

        // Guard against NaN/inf in the filter state (from bad input): start clean.
        let st = &self.state;
        if !(st[0].ic1.is_finite()
            && st[0].ic2.is_finite()
            && st[1].ic1.is_finite()
            && st[1].ic2.is_finite())
        {
            self.bypass();
            return;
        }

        if wet_target == 0.0 && self.wet < WET_FLOOR {
            // Faded out: bypass (exactly dry from the next block) and, if the knob is
            // already on the other side, wake up in the new mode next block.
            self.bypass();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn sine(hz: f32, n: usize, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / RATE as f32).sin() * amp)
            .collect()
    }

    /// Steady-state gain (dB) of a filter held at `value` for a sine at `hz`.
    fn response_db(value: f32, hz: f32) -> f32 {
        let mut f = Filter::new(RATE);
        f.set_position(value);
        let n = RATE as usize; // 1 s: settle for half, measure the second half
        let input = sine(hz, n, 0.5);
        let mut l = input.clone();
        let mut r = input.clone();
        for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            f.process(cl, cr);
        }
        let rms = |s: &[f32]| (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt();
        20.0 * (rms(&l[n / 2..]) / rms(&input[n / 2..])).log10()
    }

    #[test]
    fn centred_and_dead_zone_are_bit_identical() {
        for value in [0.5, 0.5 - DEAD_ZONE * 0.9, 0.5 + DEAD_ZONE] {
            let mut f = Filter::new(RATE);
            f.set_position(value);
            let input = sine(440.0, 4096, 0.7);
            let mut l = input.clone();
            let mut r = input.clone();
            f.process(&mut l, &mut r);
            assert_eq!(l, input, "{value}");
            assert_eq!(r, input, "{value}");
        }
    }

    #[test]
    fn returns_to_bit_identical_after_centring() {
        let mut f = Filter::new(RATE);
        f.set_position(0.1);
        let mut l = sine(440.0, 4096, 0.7);
        let mut r = l.clone();
        f.process(&mut l, &mut r);
        f.set_position(0.5);
        for _ in 0..10 {
            let mut l = sine(440.0, 4096, 0.7);
            let mut r = l.clone();
            f.process(&mut l, &mut r);
        }
        let input = sine(440.0, 1024, 0.7);
        let mut l = input.clone();
        let mut r = input.clone();
        f.process(&mut l, &mut r);
        assert_eq!(l, input);
    }

    #[test]
    fn fully_left_is_low_pass() {
        let cut = response_db(0.0, 5_000.0);
        let pass = response_db(0.0, 60.0);
        assert!(cut < -30.0, "5 kHz: {cut} dB");
        assert!(pass.abs() < 1.0, "60 Hz: {pass} dB");
    }

    #[test]
    fn fully_right_is_high_pass() {
        let cut = response_db(1.0, 60.0);
        let pass = response_db(1.0, 10_000.0);
        assert!(cut < -30.0, "60 Hz: {cut} dB");
        assert!(pass.abs() < 1.0, "10 kHz: {pass} dB");
    }

    #[test]
    fn cutoff_is_logarithmic() {
        let hz = |v: f32| match mode(v) {
            FilterMode::LowPass(f) | FilterMode::HighPass(f) => f,
            FilterMode::Off => panic!("off at {v}"),
        };
        // Just outside the dead zone it starts at the open end; at the ends it is closed.
        assert!((hz(0.5 - DEAD_ZONE - 1e-6) - LPF_OPEN_HZ).abs() < 1.0);
        assert!((hz(0.0) - LPF_CLOSED_HZ).abs() < 0.01);
        assert!((hz(1.0) - HPF_CLOSED_HZ).abs() < 0.1);
        for side in [-1.0f32, 1.0] {
            let steps: Vec<f32> = (0..=4)
                .map(|i| hz(0.5 + side * (DEAD_ZONE + 0.001 + i as f32 * 0.1)))
                .collect();
            let ratios: Vec<f32> = steps.windows(2).map(|w| w[1] / w[0]).collect();
            for r in &ratios {
                assert!((r / ratios[0] - 1.0).abs() < 1e-3, "{side}: {ratios:?}");
            }
        }
    }

    #[test]
    fn fast_sweep_across_the_centre_has_no_jumps() {
        let n = RATE as usize / 2;
        let input = sine(440.0, n, 0.8);
        let max_in = input
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        let mut f = Filter::new(RATE);
        let mut l = input.clone();
        let mut r = input.clone();
        // Sweep from fully left to fully right over ~0.2 s, one position per 64 samples.
        let chunks = l.len() / 64;
        let sweep = (RATE as usize / 5) / 64;
        for (i, (cl, cr)) in l.chunks_mut(64).zip(r.chunks_mut(64)).enumerate() {
            let t = (i as f32 / sweep as f32).min(1.0);
            f.set_position(t);
            f.process(cl, cr);
        }
        assert!(chunks > sweep);
        let max_out = l
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        // The gentle resonant peak can raise the level by up to ~1 dB (also with the knob held
        // still), so allow that; a click or zipper step would be far larger.
        let limit = max_in * 10f32.powf(1.0 / 20.0);
        assert!(
            max_out <= limit,
            "out step {max_out} > limit {limit} (in step {max_in})"
        );
    }

    #[test]
    fn peak_gain_stays_under_three_db() {
        for i in 0..=20 {
            let v = i as f32 / 20.0;
            for hz in [
                30.0, 60.0, 100.0, 150.0, 300.0, 1_000.0, 3_000.0, 8_000.0, 15_000.0,
            ] {
                let db = response_db(v, hz);
                assert!(db < 3.0, "position {v}, {hz} Hz: {db} dB");
            }
        }
    }

    #[test]
    fn non_finite_input_recovers() {
        let mut f = Filter::new(RATE);
        f.set_position(0.2);
        let mut l = vec![f32::NAN; 64];
        let mut r = vec![0.0; 64];
        f.process(&mut l, &mut r);
        let mut l = sine(440.0, 4096, 0.5);
        let mut r = l.clone();
        f.process(&mut l, &mut r);
        assert!(l.iter().chain(&r).all(|v| v.is_finite()));
    }

    #[test]
    fn describe_names_type_and_cutoff() {
        assert_eq!(describe(0.5), "off");
        assert!(describe(0.0).starts_with("LPF 100 Hz"), "{}", describe(0.0));
        assert_eq!(describe(1.0), "HPF 8.0 kHz");
        assert!(describe(0.3).starts_with("LPF "));
    }
}
