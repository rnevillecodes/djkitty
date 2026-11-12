//! Shared DSP helpers: mono mixdown, decimation, STFT and biquad filters.

use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

/// Mix interleaved stereo down to mono.
pub fn to_mono(stereo: &[f32]) -> Vec<f32> {
    stereo
        .as_chunks::<2>()
        .0
        .iter()
        .map(|f| 0.5 * (f[0] + f[1]))
        .collect()
}

/// Reduce the sample rate by an integer factor chosen to land near `target` Hz, using a
/// simple averaging (box) filter. Returns the decimated signal and its rate.
pub fn decimate_towards(mono: &[f32], rate: u32, target: u32) -> (Vec<f32>, f32) {
    let factor = (rate as f32 / target as f32).round().max(1.0) as usize;
    if factor == 1 {
        return (mono.to_vec(), rate as f32);
    }
    let out = mono
        .chunks(factor)
        .map(|c| c.iter().sum::<f32>() / c.len() as f32)
        .collect();
    (out, rate as f32 / factor as f32)
}

/// Short-time Fourier transform magnitudes with a Hann window.
pub struct Stft {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    pub size: usize,
    pub hop: usize,
}

impl Stft {
    pub fn new(size: usize, hop: usize) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(size);
        let window = (0..size)
            .map(|i| {
                let x = std::f32::consts::PI * 2.0 * i as f32 / size as f32;
                0.5 - 0.5 * x.cos()
            })
            .collect();
        Self {
            fft,
            window,
            size,
            hop,
        }
    }

    /// Number of frames for a signal of `len` samples.
    pub fn frames(&self, len: usize) -> usize {
        if len < self.size {
            usize::from(len > 0)
        } else {
            1 + (len - self.size) / self.hop
        }
    }

    /// Call `f(frame_index, magnitudes)` for each frame; magnitudes cover bins
    /// `0..=size/2`.
    pub fn for_each_frame(&self, signal: &[f32], mut f: impl FnMut(usize, &[f32])) {
        let mut buf = vec![Complex32::new(0.0, 0.0); self.size];
        let mut scratch = vec![Complex32::new(0.0, 0.0); self.fft.get_inplace_scratch_len()];
        let mut mags = vec![0.0f32; self.size / 2 + 1];
        for t in 0..self.frames(signal.len()) {
            let start = t * self.hop;
            for (i, c) in buf.iter_mut().enumerate() {
                let s = signal.get(start + i).copied().unwrap_or(0.0);
                *c = Complex32::new(s * self.window[i], 0.0);
            }
            self.fft.process_with_scratch(&mut buf, &mut scratch);
            for (m, c) in mags.iter_mut().zip(&buf) {
                *m = c.norm();
            }
            f(t, &mags);
        }
    }
}

/// A second-order IIR filter (RBJ cookbook), direct form I.
#[derive(Clone, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn from_coeffs(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn lowpass(rate: f32, freq: f32) -> Self {
        let (cos, alpha) = Self::params(rate, freq);
        Self::from_coeffs(
            (1.0 - cos) / 2.0,
            1.0 - cos,
            (1.0 - cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    pub fn highpass(rate: f32, freq: f32) -> Self {
        let (cos, alpha) = Self::params(rate, freq);
        Self::from_coeffs(
            (1.0 + cos) / 2.0,
            -(1.0 + cos),
            (1.0 + cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    fn params(rate: f32, freq: f32) -> (f32, f32) {
        let w0 = 2.0 * std::f32::consts::PI * (freq / rate).min(0.49);
        let q = std::f32::consts::FRAC_1_SQRT_2;
        (w0.cos(), w0.sin() / (2.0 * q))
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate).sin())
            .collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn lowpass_passes_low_and_cuts_high() {
        let rate = 44_100.0;
        let mut lp = Biquad::lowpass(rate, 250.0);
        let low: Vec<f32> = sine(60.0, rate, 44_100)
            .iter()
            .map(|&x| lp.process(x))
            .collect();
        let mut lp = Biquad::lowpass(rate, 250.0);
        let high: Vec<f32> = sine(8_000.0, rate, 44_100)
            .iter()
            .map(|&x| lp.process(x))
            .collect();
        assert!(rms(&low[4410..]) > 0.6);
        assert!(rms(&high[4410..]) < 0.01);
    }

    #[test]
    fn stft_finds_peak_bin() {
        let rate = 8_000.0;
        let stft = Stft::new(1024, 512);
        let x = sine(1_000.0, rate, 8_000);
        let mut peak_bin = 0;
        stft.for_each_frame(&x, |t, m| {
            if t == 3 {
                peak_bin = m
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap()
                    .0;
            }
        });
        assert_eq!(peak_bin, 128); // 1000 Hz / (8000/1024)
    }

    #[test]
    fn decimation_halves_44k() {
        let (out, r) = decimate_towards(&[0.0; 1000], 44_100, 22_050);
        assert_eq!((out.len(), r), (500, 22_050.0));
    }
}
