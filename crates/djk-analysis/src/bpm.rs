//! Tempo estimation: spectral-flux onset envelope, autocorrelation over 60–200 BPM with a
//! tempo prior, refined over several beat periods.

use crate::dsp::{Stft, decimate_towards, to_mono};

pub const MIN_BPM: f32 = 60.0;
pub const MAX_BPM: f32 = 200.0;
const ANALYSIS_RATE: u32 = 22_050;
const FRAME: usize = 1024;
const HOP: usize = 128;
/// Minimum normalised autocorrelation at the chosen lag to trust the estimate.
const MIN_PEAK: f32 = 0.2;
/// The chosen lag must stand out from the average over the search range by this ratio.
const MIN_PROMINENCE: f32 = 1.8;
/// Minimum 99th-percentile onset strength: steady tones and pads produce tiny, periodic
/// flux (STFT phase artefacts) that must not be mistaken for a beat. Real beats are ~100×
/// stronger thanks to the log compression.
const MIN_ONSET_STRENGTH: f32 = 5.0;

/// Estimate the tempo of interleaved stereo audio. `None` when there is no clear pulse.
pub fn detect_bpm(stereo: &[f32], rate: u32) -> Option<f32> {
    let mono = to_mono(stereo);
    let (signal, sr) = decimate_towards(&mono, rate, ANALYSIS_RATE);
    let env = onset_envelope(&signal);
    let fps = sr / HOP as f32;
    tempo_from_envelope(&env, fps)
}

/// Half-wave rectified, log-compressed spectral flux with the local mean removed.
fn onset_envelope(signal: &[f32]) -> Vec<f32> {
    let stft = Stft::new(FRAME, HOP);
    let mut prev: Vec<f32> = vec![0.0; FRAME / 2 + 1];
    let mut flux = Vec::with_capacity(stft.frames(signal.len()));
    stft.for_each_frame(signal, |t, mags| {
        let mut sum = 0.0;
        for (p, &m) in prev.iter_mut().zip(mags) {
            let v = (1.0 + 100.0 * m).ln();
            if t > 0 {
                sum += (v - *p).max(0.0);
            }
            *p = v;
        }
        flux.push(sum);
    });
    // Subtract a moving average (~0.4 s) and rectify.
    let w = 64usize;
    let mut out = vec![0.0; flux.len()];
    let mut acc = 0.0;
    for i in 0..flux.len() {
        acc += flux[i];
        if i >= w {
            acc -= flux[i - w];
        }
        let mean = acc / (i.min(w - 1) + 1) as f32;
        out[i] = (flux[i] - mean).max(0.0);
    }
    out
}

fn autocorr(env: &[f32], lag: usize) -> f32 {
    if lag >= env.len() {
        return 0.0;
    }
    env[..env.len() - lag]
        .iter()
        .zip(&env[lag..])
        .map(|(a, b)| a * b)
        .sum()
}

/// Parabolic interpolation of a peak at integer index `i` given neighbours.
fn parabolic(ym1: f32, y0: f32, y1: f32) -> f32 {
    let denom = ym1 - 2.0 * y0 + y1;
    if denom.abs() < 1e-12 {
        0.0
    } else {
        (0.5 * (ym1 - y1) / denom).clamp(-0.5, 0.5)
    }
}

/// Log-Gaussian tempo prior centred at 120 BPM.
fn prior(bpm: f32) -> f32 {
    let x = (bpm / 120.0).log2() / 0.9;
    (-0.5 * x * x).exp()
}

fn tempo_from_envelope(env: &[f32], fps: f32) -> Option<f32> {
    if env.len() < (fps * 4.0) as usize {
        return None;
    }
    let mut sorted = env.to_vec();
    sorted.sort_by(f32::total_cmp);
    if sorted[sorted.len() * 99 / 100] < MIN_ONSET_STRENGTH {
        return None;
    }
    let r0 = autocorr(env, 0);
    if r0 <= 1e-9 {
        return None;
    }
    let min_lag = (60.0 * fps / MAX_BPM).floor() as usize;
    let max_lag = (60.0 * fps / MIN_BPM).ceil() as usize;
    let r: Vec<f32> = (0..=max_lag + 1)
        .map(|lag| autocorr(env, lag) / r0)
        .collect();

    // Pick the lag with the best prior-weighted score among local maxima.
    let mut best: Option<(usize, f32)> = None;
    for lag in min_lag.max(2)..=max_lag {
        if r[lag] < r[lag - 1] || r[lag] < r[lag + 1] {
            continue;
        }
        let bpm = 60.0 * fps / lag as f32;
        let score = r[lag] * prior(bpm);
        if best.is_none_or(|(_, s)| score > s) {
            best = Some((lag, score));
        }
    }
    let (lag, _) = best?;

    // Confidence: peak height and prominence over the search range.
    let mean = r[min_lag..=max_lag].iter().sum::<f32>() / (max_lag - min_lag + 1) as f32;
    if r[lag] < MIN_PEAK || r[lag] < MIN_PROMINENCE * mean.max(1e-6) {
        return None;
    }

    // Refine the period using peaks near k × lag (precision improves with k).
    let mut sum = 0.0;
    let mut weight = 0.0;
    for k in 1..=8usize {
        let centre = lag * k;
        if centre + 2 >= env.len() / 2 {
            break;
        }
        let search = k + 1;
        let (lo, hi) = (centre.saturating_sub(search).max(1), centre + search);
        let vals: Vec<(usize, f32)> = (lo..=hi).map(|l| (l, autocorr(env, l))).collect();
        let &(pk, _) = vals.iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
        let off = parabolic(
            autocorr(env, pk - 1),
            autocorr(env, pk),
            autocorr(env, pk + 1),
        );
        let period = (pk as f32 + off) / k as f32;
        let w = k as f32;
        sum += period * w;
        weight += w;
    }
    let period = if weight > 0.0 {
        sum / weight
    } else {
        lag as f32
    };
    let bpm = 60.0 * fps / period;
    (MIN_BPM..=MAX_BPM).contains(&bpm).then_some(bpm)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A click track: short decaying noise bursts on every beat, plus a quieter hi-hat on
    /// the off-beat.
    fn click_track(bpm: f32, secs: f32, rate: u32) -> Vec<f32> {
        let n = (secs * rate as f32) as usize;
        let mut out = vec![0.0f32; n * 2];
        let period = 60.0 / bpm * rate as f32;
        let mut seed = 12345u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        let mut beat = 0.0f32;
        while (beat as usize) < n {
            let start = beat as usize;
            for i in 0..(rate as usize / 50) {
                if start + i >= n {
                    break;
                }
                let env = (-(i as f32) / (rate as f32 * 0.004)).exp();
                let kick = (2.0 * std::f32::consts::PI * 55.0 * i as f32 / rate as f32).sin();
                let v = 0.8 * env * (0.6 * kick + 0.4 * noise());
                out[(start + i) * 2] += v;
                out[(start + i) * 2 + 1] += v;
            }
            beat += period;
        }
        out
    }

    fn assert_tempo(true_bpm: f32) {
        let audio = click_track(true_bpm, 30.0, 44_100);
        let bpm = detect_bpm(&audio, 44_100).expect("tempo detected");
        let ok = [true_bpm, true_bpm / 2.0, true_bpm * 2.0]
            .iter()
            .any(|&t| (bpm - t).abs() <= 1.0 * (t / true_bpm).max(0.5));
        assert!(ok, "true {true_bpm}, got {bpm}");
    }

    #[test]
    fn detects_90() {
        assert_tempo(90.0);
    }

    #[test]
    fn detects_120() {
        assert_tempo(120.0);
    }

    #[test]
    fn detects_128() {
        assert_tempo(128.0);
    }

    #[test]
    fn detects_174() {
        assert_tempo(174.0);
    }

    #[test]
    fn noise_is_unknown() {
        let mut seed = 99u32;
        let audio: Vec<f32> = (0..44_100 * 20)
            .flat_map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let v = ((seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0) * 0.3;
                [v, v]
            })
            .collect();
        assert_eq!(detect_bpm(&audio, 44_100), None);
    }

    #[test]
    fn steady_tone_is_unknown() {
        // A pure sustained sine has no beat (regression: phase artefacts looked periodic).
        let audio: Vec<f32> = (0..44_100 * 30)
            .flat_map(|i| {
                let v = (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 44_100.0).sin() * 0.1;
                [v, v]
            })
            .collect();
        assert_eq!(detect_bpm(&audio, 44_100), None);
    }

    #[test]
    fn silence_is_unknown() {
        assert_eq!(detect_bpm(&vec![0.0; 44_100 * 20], 44_100), None);
    }
}
