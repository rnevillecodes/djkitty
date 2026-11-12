//! Key estimation: averaged chromagram correlated with Krumhansl–Schmuckler profiles.

use djk_core::{Key, Mode};

use crate::dsp::{Stft, decimate_towards, to_mono};

const ANALYSIS_RATE: u32 = 22_050;
const FRAME: usize = 8192;
const HOP: usize = 4096;
const MIN_HZ: f32 = 50.0;
const MAX_HZ: f32 = 5_000.0;
/// Minimum correlation of the best key.
const MIN_CORRELATION: f32 = 0.5;
/// Minimum margin between the best key and the best key of a different tonic/mode that
/// is not its relative.
const MIN_MARGIN: f32 = 0.02;

const MAJOR: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// Estimate the key of interleaved stereo audio. `None` when not confident.
pub fn detect_key(stereo: &[f32], rate: u32) -> Option<Key> {
    let chroma = chromagram(stereo, rate)?;
    key_from_chroma(&chroma)
}

/// Average pitch-class profile over the track (12 values, C first).
pub fn chromagram(stereo: &[f32], rate: u32) -> Option<[f32; 12]> {
    let mono = to_mono(stereo);
    let (signal, sr) = decimate_towards(&mono, rate, ANALYSIS_RATE);
    let stft = Stft::new(FRAME, HOP);
    let bin_hz = sr / FRAME as f32;
    // Precompute bin → pitch class.
    let classes: Vec<Option<usize>> = (0..=FRAME / 2)
        .map(|b| {
            let f = b as f32 * bin_hz;
            (MIN_HZ..=MAX_HZ).contains(&f).then(|| {
                let midi = 69.0 + 12.0 * (f / 440.0).log2();
                (midi.round() as i32).rem_euclid(12) as usize
            })
        })
        .collect();
    let mut total = [0.0f32; 12];
    stft.for_each_frame(&signal, |_, mags| {
        let mut frame = [0.0f32; 12];
        for (m, c) in mags.iter().zip(&classes) {
            if let Some(c) = c {
                frame[*c] += m * m;
            }
        }
        let norm: f32 = frame.iter().sum();
        if norm > 1e-9 {
            for (t, f) in total.iter_mut().zip(frame) {
                *t += f / norm;
            }
        }
    });
    (total.iter().sum::<f32>() > 1e-6).then_some(total)
}

fn pearson(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let ma = a.iter().sum::<f32>() / 12.0;
    let mb = b.iter().sum::<f32>() / 12.0;
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        let (x, y) = (a[i] - ma, b[i] - mb);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da <= 0.0 || db <= 0.0 {
        0.0
    } else {
        num / (da.sqrt() * db.sqrt())
    }
}

fn rotated(profile: &[f32; 12], tonic: usize) -> [f32; 12] {
    let mut out = [0.0; 12];
    for (i, o) in out.iter_mut().enumerate() {
        *o = profile[(i + 12 - tonic) % 12];
    }
    out
}

/// Correlate a chroma vector with all 24 key profiles.
pub fn key_from_chroma(chroma: &[f32; 12]) -> Option<Key> {
    let mut scores: Vec<(Key, f32)> = Vec::with_capacity(24);
    for tonic in 0..12u8 {
        scores.push((
            Key::new(tonic, Mode::Major),
            pearson(chroma, &rotated(&MAJOR, tonic as usize)),
        ));
        scores.push((
            Key::new(tonic, Mode::Minor),
            pearson(chroma, &rotated(&MINOR, tonic as usize)),
        ));
    }
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (best, best_score) = scores[0];
    if best_score < MIN_CORRELATION {
        return None;
    }
    // Relative major/minor share notes; compare against the best unrelated key.
    let runner_up = scores[1..]
        .iter()
        .find(|(k, _)| k.camelot_number() != best.camelot_number())
        .map_or(0.0, |s| s.1);
    (best_score - runner_up >= MIN_MARGIN).then_some(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn midi_hz(m: f32) -> f32 {
        440.0 * 2f32.powf((m - 69.0) / 12.0)
    }

    /// Render a chord progression (MIDI note lists, 2 s each) with a few harmonics.
    fn progression(chords: &[&[u8]], rate: u32) -> Vec<f32> {
        let per = rate as usize * 2;
        let mut out = Vec::with_capacity(per * chords.len() * 2);
        for chord in chords {
            for i in 0..per {
                let t = i as f32 / rate as f32;
                let mut v = 0.0;
                for &n in *chord {
                    let f = midi_hz(f32::from(n));
                    for h in 1..=3 {
                        v += (2.0 * std::f32::consts::PI * f * h as f32 * t).sin() / h as f32;
                    }
                }
                let v = v * 0.1;
                out.push(v);
                out.push(v);
            }
        }
        out
    }

    #[test]
    fn detects_a_minor() {
        // Am - Dm - E - Am, with the tonic doubled in the bass.
        let chords: [&[u8]; 4] = [
            &[45, 57, 60, 64],
            &[50, 57, 62, 65],
            &[52, 56, 59, 64],
            &[45, 57, 60, 64],
        ];
        let key = detect_key(&progression(&chords, 22_050), 22_050).expect("key");
        assert_eq!(key, Key::new(9, Mode::Minor), "{}", key.standard());
        assert_eq!(key.camelot(), "8A");
    }

    #[test]
    fn detects_c_major() {
        // C - F - G - C
        let chords: [&[u8]; 4] = [
            &[48, 60, 64, 67],
            &[53, 60, 65, 69],
            &[55, 59, 62, 67],
            &[48, 60, 64, 67],
        ];
        let key = detect_key(&progression(&chords, 22_050), 22_050).expect("key");
        assert_eq!(key, Key::new(0, Mode::Major), "{}", key.standard());
    }

    #[test]
    fn noise_is_unknown() {
        let mut seed = 7u32;
        let audio: Vec<f32> = (0..22_050 * 10)
            .flat_map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let v = ((seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0) * 0.3;
                [v, v]
            })
            .collect();
        assert_eq!(detect_key(&audio, 22_050), None);
    }
}
