//! Three-band waveform data: per-band peak levels at a fixed number of bins per second,
//! stored as a compact binary cache file.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::dsp::Biquad;

/// Detail resolution: bins per second of audio.
pub const BINS_PER_SEC: u32 = 600;
/// Approximate number of bins in a track overview.
pub const OVERVIEW_BINS: usize = 4000;
/// Low/mid crossover in Hz.
pub const LOW_MID_HZ: f32 = 250.0;
/// Mid/high crossover in Hz.
pub const MID_HIGH_HZ: f32 = 4_000.0;

const MAGIC: &[u8; 4] = b"DJKW";
const VERSION: u8 = 1;

/// Per-band peak levels (0â€“255) for one bin: `[low, mid, high]`.
pub type Bin = [u8; 3];

#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    pub bins_per_sec: u32,
    pub bins: Vec<Bin>,
}

impl Waveform {
    /// Compute from interleaved stereo audio at `rate`.
    pub fn from_audio(stereo: &[f32], rate: u32) -> Self {
        let frames = stereo.len() / 2;
        let rate_f = rate as f32;
        let samples_per_bin = rate as f64 / f64::from(BINS_PER_SEC);
        let n_bins = (frames as f64 / samples_per_bin).ceil() as usize;

        let mut low = Biquad::lowpass(rate_f, LOW_MID_HZ);
        let mut mid_hp = Biquad::highpass(rate_f, LOW_MID_HZ);
        let mut mid_lp = Biquad::lowpass(rate_f, MID_HIGH_HZ);
        let mut high = Biquad::highpass(rate_f, MID_HIGH_HZ);

        let mut bins = Vec::with_capacity(n_bins);
        let mut peaks = [0.0f32; 3];
        let mut next_edge = samples_per_bin;
        for (i, f) in stereo.as_chunks::<2>().0.iter().enumerate() {
            let x = 0.5 * (f[0] + f[1]);
            let l = low.process(x).abs();
            let m = mid_lp.process(mid_hp.process(x)).abs();
            let h = high.process(x).abs();
            peaks[0] = peaks[0].max(l);
            peaks[1] = peaks[1].max(m);
            peaks[2] = peaks[2].max(h);
            if (i + 1) as f64 >= next_edge {
                bins.push(quantise(peaks));
                peaks = [0.0; 3];
                next_edge += samples_per_bin;
            }
        }
        if bins.len() < n_bins {
            bins.push(quantise(peaks));
        }
        Self {
            bins_per_sec: BINS_PER_SEC,
            bins,
        }
    }

    pub fn duration_secs(&self) -> f64 {
        self.bins.len() as f64 / f64::from(self.bins_per_sec.max(1))
    }

    /// Max-reduce to about `target` bins for a whole-track overview.
    pub fn overview(&self, target: usize) -> Vec<Bin> {
        if self.bins.is_empty() || target == 0 {
            return Vec::new();
        }
        let per = self.bins.len().div_ceil(target).max(1);
        self.bins
            .chunks(per)
            .map(|c| {
                c.iter().fold([0u8; 3], |acc, b| {
                    [acc[0].max(b[0]), acc[1].max(b[1]), acc[2].max(b[2])]
                })
            })
            .collect()
    }

    pub fn write_to(&self, mut w: impl Write) -> std::io::Result<()> {
        w.write_all(MAGIC)?;
        w.write_all(&[VERSION])?;
        w.write_all(&(self.bins_per_sec as u16).to_le_bytes())?;
        w.write_all(&(self.bins.len() as u32).to_le_bytes())?;
        let flat: Vec<u8> = self.bins.iter().flatten().copied().collect();
        w.write_all(&flat)
    }

    pub fn read_from(mut r: impl Read) -> std::io::Result<Self> {
        let bad = |msg: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, msg.to_owned());
        let mut header = [0u8; 11];
        r.read_exact(&mut header)?;
        if &header[..4] != MAGIC {
            return Err(bad("not a djkitty waveform file"));
        }
        if header[4] != VERSION {
            return Err(bad("unsupported waveform version"));
        }
        let bins_per_sec = u32::from(u16::from_le_bytes([header[5], header[6]]));
        let n = u32::from_le_bytes([header[7], header[8], header[9], header[10]]) as usize;
        let mut flat = vec![0u8; n * 3];
        r.read_exact(&mut flat)?;
        let bins = flat.as_chunks::<3>().0.to_vec();
        Ok(Self { bins_per_sec, bins })
    }

    /// Write atomically (temp file + rename).
    /// Write the cache file atomically. The loader and the analysis workers can save the
    /// same track at once, so each save writes its own temporary file before renaming it
    /// into place.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!("wf.{}-{n}.tmp", std::process::id()));
        let result = (|| {
            let f = std::fs::File::create(&tmp)?;
            let mut w = std::io::BufWriter::new(f);
            self.write_to(&mut w)?;
            w.flush()?;
            drop(w);
            std::fs::rename(&tmp, path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let f = std::fs::File::open(path)?;
        Self::read_from(std::io::BufReader::new(f))
    }
}

fn quantise(p: [f32; 3]) -> Bin {
    p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, rate: u32, secs: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        (0..n)
            .flat_map(|i| {
                let v = (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin() * 0.8;
                [v, v]
            })
            .collect()
    }

    fn band_means(w: &Waveform) -> [f32; 3] {
        let body = &w.bins[w.bins.len() / 4..w.bins.len() * 3 / 4];
        let mut m = [0.0f32; 3];
        for b in body {
            for k in 0..3 {
                m[k] += f32::from(b[k]);
            }
        }
        m.map(|v| v / body.len() as f32)
    }

    #[test]
    fn low_tone_lands_in_low_band() {
        let w = Waveform::from_audio(&tone(60.0, 44_100, 2.0), 44_100);
        let [l, m, h] = band_means(&w);
        assert!(l > 150.0, "low {l}");
        assert!(l > 3.0 * m && l > 10.0 * h.max(1.0), "{l} {m} {h}");
    }

    #[test]
    fn high_tone_lands_in_high_band() {
        let w = Waveform::from_audio(&tone(8_000.0, 44_100, 2.0), 44_100);
        let [l, m, h] = band_means(&w);
        assert!(h > 150.0, "high {h}");
        assert!(h > 3.0 * m && h > 10.0 * l.max(1.0), "{l} {m} {h}");
    }

    #[test]
    fn bin_count_matches_duration() {
        for (rate, secs) in [(44_100, 3.0f32), (48_000, 2.5), (44_100, 0.01)] {
            let w = Waveform::from_audio(&tone(440.0, rate, secs), rate);
            let expected = (f64::from(secs) * f64::from(BINS_PER_SEC)).ceil() as usize;
            assert!(
                w.bins.len().abs_diff(expected) <= 1,
                "{rate} {secs}: {} vs {expected}",
                w.bins.len()
            );
        }
    }

    #[test]
    fn overview_reduces_and_keeps_peaks() {
        let mut w = Waveform {
            bins_per_sec: BINS_PER_SEC,
            bins: vec![[1, 2, 3]; 180_000],
        };
        w.bins[123] = [200, 0, 0];
        let o = w.overview(OVERVIEW_BINS);
        assert!(o.len() <= OVERVIEW_BINS && o.len() > OVERVIEW_BINS * 9 / 10);
        assert!(o.iter().any(|b| b[0] == 200));
    }

    #[test]
    fn roundtrips_through_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("1.wf");
        let w = Waveform::from_audio(&tone(440.0, 44_100, 1.0), 44_100);
        w.save(&path).unwrap();
        assert_eq!(Waveform::load(&path).unwrap(), w);
        std::fs::write(&path, b"nope").unwrap();
        assert!(Waveform::load(&path).is_err());
    }
}
