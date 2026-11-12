//! Integrated loudness in LUFS, measured as in ITU-R BS.1770-4 / EBU R128.
//!
//! LUFS tells you how loud a track *sounds* (unlike peak level), so a DJ can see which
//! tracks need the channel fader or trim pulled down to blend at the same level.
//!
//! The measurement:
//! 1. **K-weighting**: a high-shelf (about +4 dB above 2 kHz, like the head's effect on
//!    sound) followed by a high-pass at about 38 Hz.
//! 2. **Blocks**: mean square per channel over 400 ms blocks that overlap by 75%,
//!    summed over left and right.
//! 3. **Gating**: blocks quieter than −70 LUFS (silence) are dropped, then blocks more
//!    than 10 LU below the average of what is left (quiet intros, breakdowns) are dropped.
//! 4. The average power of the remaining blocks, as `−0.691 + 10·log10(power)`.
//!
//! Tracks are decoded to stereo, so a mono file counts as the same signal on both
//! speakers, which is how it plays.

/// Block length and step, in seconds.
const BLOCK_SECS: f64 = 0.4;
const STEP_SECS: f64 = 0.1;
/// Blocks quieter than this are silence and never count.
const ABSOLUTE_GATE_LUFS: f64 = -70.0;
/// Blocks this far below the ungated average are dropped.
const RELATIVE_GATE_LU: f64 = 10.0;

/// Integrated loudness of interleaved stereo audio, or `None` when it is too short (under
/// one 400 ms block) or silent.
pub fn integrated_lufs(stereo: &[f32], rate: u32) -> Option<f32> {
    if rate == 0 {
        return None;
    }
    let frames = stereo.len() / 2;
    let block = (BLOCK_SECS * f64::from(rate)).round() as usize;
    let step = (STEP_SECS * f64::from(rate)).round() as usize;
    if frames < block || step == 0 {
        return None;
    }

    // Squared K-weighted samples summed over both channels, in 100 ms steps; each block
    // is then the sum of four steps.
    let mut filters = [KWeighting::new(rate), KWeighting::new(rate)];
    let mut steps = Vec::with_capacity(frames / step + 1);
    let mut acc = 0.0f64;
    let mut n = 0;
    for frame in stereo.as_chunks::<2>().0 {
        for (ch, &x) in frame.iter().enumerate() {
            let y = filters[ch].process(f64::from(x));
            acc += y * y;
        }
        n += 1;
        if n == step {
            steps.push(acc);
            acc = 0.0;
            n = 0;
        }
    }
    let per_block = block / step;
    let blocks: Vec<f64> = steps
        .windows(per_block)
        .map(|w| w.iter().sum::<f64>() / (per_block * step) as f64)
        .collect();

    let absolute = power_of(ABSOLUTE_GATE_LUFS);
    let loud: Vec<f64> = blocks.into_iter().filter(|&p| p > absolute).collect();
    if loud.is_empty() {
        return None;
    }
    let relative = mean(&loud) * power_of(-RELATIVE_GATE_LU);
    let gated: Vec<f64> = loud.into_iter().filter(|&p| p > relative).collect();
    Some(lufs_of(mean(&gated)) as f32)
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn lufs_of(power: f64) -> f64 {
    -0.691 + 10.0 * power.log10()
}

/// Inverse of [`lufs_of`] (for a loudness *difference*, the 0.691 cancels out).
fn power_of(lufs: f64) -> f64 {
    10f64.powf((lufs + 0.691) / 10.0)
}

/// Direct-form-I biquad in `f64` (the high-pass pole sits very close to 1).
#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 2]) -> Self {
        Self {
            b,
            a,
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// The two K-weighting stages, designed for any sample rate (at 48 kHz they match the
/// coefficients printed in BS.1770).
struct KWeighting {
    shelf: Biquad,
    highpass: Biquad,
}

impl KWeighting {
    fn new(rate: u32) -> Self {
        let rate = f64::from(rate);
        // Stage 1: high shelf.
        let (f0, gain_db, q) = (
            1_681.974_450_955_533,
            3.999_843_853_973_347,
            0.707_175_236_955_419_6,
        );
        let k = (std::f64::consts::PI * f0 / rate).tan();
        let vh = 10f64.powf(gain_db / 20.0);
        let vb = vh.powf(0.499_666_774_154_541_6);
        let a0 = 1.0 + k / q + k * k;
        let shelf = Biquad::new(
            [
                (vh + vb * k / q + k * k) / a0,
                2.0 * (k * k - vh) / a0,
                (vh - vb * k / q + k * k) / a0,
            ],
            [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        );
        // Stage 2: high-pass.
        let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
        let k = (std::f64::consts::PI * f0 / rate).tan();
        let a0 = 1.0 + k / q + k * k;
        let highpass = Biquad::new(
            [1.0, -2.0, 1.0],
            [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        );
        Self { shelf, highpass }
    }

    fn process(&mut self, x: f64) -> f64 {
        self.highpass.process(self.shelf.process(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, amp: f32, secs: f32, rate: u32) -> Vec<f32> {
        let n = (secs * rate as f32) as usize;
        (0..n)
            .flat_map(|i| {
                let s = amp * (std::f32::consts::TAU * freq * i as f32 / rate as f32).sin();
                [s, s]
            })
            .collect()
    }

    #[test]
    fn matches_the_k_weighting_coefficients_in_the_standard() {
        // BS.1770-4 table 1 and 2 (48 kHz).
        let k = KWeighting::new(48_000);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-8;
        assert!(close(k.shelf.b[0], 1.535_124_859_586_97));
        assert!(close(k.shelf.b[1], -2.691_696_189_406_38));
        assert!(close(k.shelf.b[2], 1.198_392_810_852_85));
        assert!(close(k.shelf.a[0], -1.690_659_293_182_41));
        assert!(close(k.shelf.a[1], 0.732_480_774_215_85));
        assert!(close(k.highpass.a[0], -1.990_047_454_833_98));
        assert!(close(k.highpass.a[1], 0.990_072_250_366_21));
    }

    #[test]
    fn a_full_scale_1khz_tone_on_both_channels_reads_about_0_lufs() {
        // Each channel alone reads −3.01 LUFS; both together add up to about 0.
        for rate in [44_100, 48_000, 96_000] {
            let lufs = integrated_lufs(&sine(997.0, 1.0, 5.0, rate), rate).unwrap();
            assert!((lufs - 0.0).abs() < 0.1, "{rate} Hz: {lufs}");
        }
    }

    #[test]
    fn halving_the_level_lowers_loudness_by_6_db() {
        let rate = 48_000;
        let loud = integrated_lufs(&sine(997.0, 0.5, 5.0, rate), rate).unwrap();
        let quiet = integrated_lufs(&sine(997.0, 0.25, 5.0, rate), rate).unwrap();
        assert!((loud - quiet - 6.02).abs() < 0.05, "{loud} vs {quiet}");
    }

    #[test]
    fn silence_and_quiet_breakdowns_are_gated_out() {
        let rate = 48_000;
        let tone = sine(997.0, 0.5, 5.0, rate);
        let alone = integrated_lufs(&tone, rate).unwrap();
        // Ten seconds of silence and a breakdown 30 dB down around the same tone.
        let mut track = vec![0.0; 10 * rate as usize * 2];
        track.extend(sine(997.0, 0.5 / 31.6, 10.0, rate));
        track.extend(&tone);
        let gated = integrated_lufs(&track, rate).unwrap();
        // The three blocks overlapping the tone's start are partly loud, so they pass the
        // gate and pull the result down slightly (about 0.13 LU). Without gating it would
        // read about −10.8 LUFS.
        assert!((gated - alone).abs() < 0.2, "{gated} vs {alone}");
    }

    #[test]
    fn too_short_or_silent_audio_has_no_loudness() {
        assert_eq!(
            integrated_lufs(&sine(997.0, 0.5, 0.2, 48_000), 48_000),
            None
        );
        assert_eq!(integrated_lufs(&vec![0.0; 96_000 * 2], 48_000), None);
        assert_eq!(integrated_lufs(&[], 48_000), None);
    }
}
