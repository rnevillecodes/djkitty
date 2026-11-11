//! Beatgrid detection: where the beats fall, and where the grid starts.
//!
//! Starting from the detected tempo, the period is fine-tuned so beats stay aligned over
//! the whole track and the phase is found by folding the onset envelope at that period.
//! The grid starts at the beat where the music becomes audible, which is bar 1 beat 1
//! (most tracks start on a downbeat; the DJ can re-phrase bars if not).

use djk_core::Beatgrid;

use crate::bpm::{ANALYSIS_RATE, HOP, onset_envelope};
use crate::dsp::{Biquad, decimate_towards, to_mono};

/// Cut-off for the signal used to find beat phase: keeps kicks and snares, drops hats.
const BEAT_LOWPASS_HZ: f32 = 1_500.0;
/// Phase histogram resolution (bins per beat).
const PHASE_BINS: usize = 64;
/// Fine-tuning range around the detected tempo (±0.25%) and step (0.01%).
const TUNE_RANGE: f64 = 0.0025;
const TUNE_STEP: f64 = 0.0001;
/// The onset envelope reports a transient slightly before it is heard, because the
/// analysis window reaches it early; this moves beat times onto the transient.
/// Calibrated on synthetic kicks (see tests).
const ONSET_DELAY_SECS: f64 = 0.038;
/// Sound counts as audible once a short window rises above this level relative to the
/// track's peak (−40 dB): well above dither and hiss, well below any real intro.
const AUDIBLE_DB: f32 = -40.0;
/// Window for the audible-start check.
const AUDIBLE_WINDOW_SECS: f64 = 0.01;

/// Detect the beatgrid of interleaved stereo audio at `bpm` (from tempo detection).
pub fn detect_grid(stereo: &[f32], rate: u32, bpm: f32) -> Option<Beatgrid> {
    let mono = to_mono(stereo);
    let audible = audible_start(&mono, rate)?;
    let (signal, sr) = decimate_towards(&mono, rate, ANALYSIS_RATE);
    // Kicks and snares mark the beat; hats on the off-beats would pull the phase half a
    // beat out, so they are filtered away first.
    let mut lp1 = Biquad::lowpass(sr, BEAT_LOWPASS_HZ);
    let mut lp2 = Biquad::lowpass(sr, BEAT_LOWPASS_HZ);
    let beat_signal: Vec<f32> = signal
        .iter()
        .map(|&x| lp2.process(lp1.process(x)))
        .collect();
    let env = onset_envelope(&beat_signal);
    let fps = f64::from(sr) / HOP as f64;
    let base_period = 60.0 * fps / f64::from(bpm);
    if env.len() < (base_period * 8.0) as usize {
        return None;
    }

    // Fine-tune the period: the right one folds the onsets into the sharpest peak. A
    // coarse sweep, then a finer one around the best step.
    let best_in = |centre: f64, range: f64, step: f64| {
        let steps = (range / step).round() as i64;
        (-steps..=steps)
            .map(|s| {
                let p = centre * (1.0 + s as f64 * step);
                (p, phase_histogram(&env, p))
            })
            .max_by(|a, b| peakiness(&a.1).total_cmp(&peakiness(&b.1)))
    };
    let (coarse, _) = best_in(base_period, TUNE_RANGE, TUNE_STEP)?;
    let (period, hist) = best_in(coarse, TUNE_STEP, TUNE_STEP / 10.0)?;

    // Phase of the beats, refined between histogram bins.
    let (peak, _) = hist.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    let at = |i: isize| hist[i.rem_euclid(PHASE_BINS as isize) as usize];
    let (ym1, y0, y1) = (
        at(peak as isize - 1),
        at(peak as isize),
        at(peak as isize + 1),
    );
    let denom = ym1 - 2.0 * y0 + y1;
    let off = if denom.abs() < 1e-12 {
        0.0
    } else {
        (0.5 * (ym1 - y1) / denom).clamp(-0.5, 0.5)
    };
    let phase_frames = (peak as f64 + 0.5 + off) / PHASE_BINS as f64 * period;
    let bpm = 60.0 * fps / period;
    let any_beat = Beatgrid::new(bpm, phase_frames / fps + ONSET_DELAY_SECS);

    // The grid starts at the beat nearest where the music becomes audible: bar 1, beat 1.
    let n = any_beat.beat_at(audible).round() as i64;
    Some(Beatgrid::new(bpm, any_beat.beat_time(n)))
}

/// When the music becomes audible: the first moment a short window rises above
/// [`AUDIBLE_DB`] relative to the track's peak. `None` for silence.
pub fn audible_start(mono: &[f32], rate: u32) -> Option<f64> {
    let peak = mono.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if peak <= 1e-6 {
        return None;
    }
    let threshold = peak * 10f32.powf(AUDIBLE_DB / 20.0);
    let window = ((AUDIBLE_WINDOW_SECS * f64::from(rate)) as usize).max(1);
    let first = mono
        .chunks(window)
        .position(|w| (w.iter().map(|v| v * v).sum::<f32>() / w.len() as f32).sqrt() > threshold)?;
    // Within that window, the first sample that crosses the threshold.
    let start = first * window;
    let offset = mono[start..]
        .iter()
        .take(window)
        .position(|v| v.abs() > threshold)
        .unwrap_or(0);
    Some((start + offset) as f64 / f64::from(rate))
}

/// Onset envelope folded at `period` frames into PHASE_BINS bins.
fn phase_histogram(env: &[f32], period: f64) -> Vec<f64> {
    let mut hist = vec![0.0; PHASE_BINS];
    for (i, &v) in env.iter().enumerate() {
        let phase = (i as f64 / period).fract();
        hist[((phase * PHASE_BINS as f64) as usize).min(PHASE_BINS - 1)] += f64::from(v);
    }
    hist
}

/// How sharply a phase histogram peaks: peak (smoothed over 3 bins) over mean.
fn peakiness(hist: &[f64]) -> f64 {
    let n = hist.len();
    let mean = hist.iter().sum::<f64>() / n as f64;
    let best = (0..n)
        .map(|i| hist[(i + n - 1) % n] + hist[i] + hist[(i + 1) % n])
        .fold(0.0, f64::max)
        / 3.0;
    best / mean.max(1e-12)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    /// A bar pattern: `(beat position, kind)` with 0 = kick, 1 = snare, 2 = hat, starting
    /// with a downbeat at `start` seconds.
    fn pattern(bpm: f64, secs: f64, start: f64, hits: &[(f64, u8)]) -> Vec<f32> {
        let n = (secs * f64::from(RATE)) as usize;
        let mut out = vec![0.0f32; n * 2];
        let beat = 60.0 / bpm;
        let mut seed = 4242u32;
        let mut noise = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        let mut bar = start;
        while bar < secs {
            for &(pos, kind) in hits {
                let at = ((bar + pos * beat) * f64::from(RATE)).round() as usize;
                for i in 0..(RATE as usize / 10) {
                    if at + i >= n {
                        break;
                    }
                    let t = i as f32 / RATE as f32;
                    let v = match kind {
                        0 => {
                            0.9 * (-t / 0.08).exp()
                                * (2.0
                                    * std::f32::consts::PI
                                    * (50.0 + 90.0 * (-t / 0.02).exp())
                                    * t)
                                    .sin()
                        }
                        1 => 0.6 * (-t / 0.05).exp() * noise(),
                        _ => 0.15 * (-t / 0.01).exp() * noise(),
                    };
                    out[(at + i) * 2] += v;
                    out[(at + i) * 2 + 1] += v;
                }
            }
            bar += 4.0 * beat;
        }
        out
    }

    fn grid_for(audio: &[f32], bpm: f64) -> Beatgrid {
        detect_grid(audio, RATE, bpm as f32).expect("grid")
    }

    /// Prints the signed timing error of each pattern, for calibrating ONSET_DELAY_SECS.
    #[test]
    #[ignore]
    fn calibrate_onset_delay() {
        for (bpm, start) in [(128.0, 0.437), (174.0, 0.21), (90.0, 0.1), (140.0, 0.3)] {
            let hits = [(0.0, 0), (1.0, 1), (2.0, 0), (3.0, 1)];
            let audio = pattern(bpm, 30.0, start, &hits);
            let g = grid_for(&audio, bpm);
            let n = g.beat_at(start).round() as i64;
            eprintln!("{bpm}: grid - true = {:+.4} s", g.beat_time(n) - start);
        }
    }

    /// Distance (seconds) from `t` to the nearest beat of `g`.
    fn beat_error(g: &Beatgrid, t: f64) -> f64 {
        let n = g.beat_at(t).round() as i64;
        (g.beat_time(n) - t).abs()
    }

    #[test]
    fn grid_starts_at_the_first_audible_beat() {
        // Kick on 1, snare on 2 and 4, hats on every 8th; the music starts at 0.437 s.
        let hits = [
            (0.0, 0),
            (1.0, 1),
            (3.0, 1),
            (0.5, 2),
            (1.5, 2),
            (2.0, 2),
            (2.5, 2),
            (3.5, 2),
        ];
        let audio = pattern(128.0, 30.0, 0.437, &hits);
        let g = grid_for(&audio, 128.0);
        assert!((g.bpm - 128.0).abs() < 0.05, "bpm {}", g.bpm);
        assert!(
            (g.start_secs - 0.437).abs() < 0.006,
            "start {}",
            g.start_secs
        );
        // Bar 1 beat 1 is where the music starts, and nothing comes before it.
        assert_eq!(g.bar_beat(0.44).map(|b| (b.bar, b.beat)), Some((1, 1)));
        assert_eq!(g.bar_beat(0.3), None);
        assert_eq!(
            g.bar_beat(0.437 + 4.0 * 60.0 / 128.0 + 0.01)
                .map(|b| b.beat),
            Some(1)
        );
    }

    #[test]
    fn a_long_silent_lead_in_is_skipped() {
        // Five seconds of silence, then a two-step groove (kicks on 1 and the "and" of 3).
        let hits = [(0.0, 0), (2.5, 0), (1.0, 1), (3.0, 1)];
        let audio = pattern(174.0, 30.0, 5.0, &hits);
        let g = grid_for(&audio, 174.0);
        assert!((g.start_secs - 5.0).abs() < 0.006, "start {}", g.start_secs);
        assert_eq!(g.beats_between(0.0, 4.9).count(), 0);
        let bar = 4.0 * 60.0 / 174.0;
        assert_eq!(
            g.bar_beat(5.0 + 3.0 * bar + 0.005).map(|b| (b.bar, b.beat)),
            Some((4, 1))
        );
    }

    #[test]
    fn audible_start_ignores_quiet_hiss() {
        // −80 dB hiss for a second, then a full-level tone.
        let rate = 1_000;
        let mut mono = vec![0.0001f32; rate];
        mono.extend(std::iter::repeat_n(0.8f32, rate));
        let t = audible_start(&mono, rate as u32).unwrap();
        assert!((t - 1.0).abs() < 0.002, "{t}");
        assert_eq!(audible_start(&[0.0; 100], rate as u32), None);
    }

    #[test]
    fn stays_aligned_to_the_end_of_a_long_track() {
        // 5 minutes at a slightly off-round tempo, given a rounded starting estimate.
        let hits = [(0.0, 0), (1.0, 0), (2.0, 0), (3.0, 0)];
        let true_bpm = 174.13;
        let audio = pattern(true_bpm, 300.0, 0.05, &hits);
        let g = grid_for(&audio, 174.0);
        let beat = 60.0 / true_bpm;
        let late = 0.05 + (280.0 / beat).round() * beat;
        assert!(
            beat_error(&g, late) < 0.01,
            "bpm {} error {}",
            g.bpm,
            beat_error(&g, late)
        );
    }
}
