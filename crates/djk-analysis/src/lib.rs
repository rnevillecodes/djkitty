//! Decoding, file stats, BPM, key and waveform analysis.

pub mod bpm;
pub mod decode;
pub mod dsp;
pub mod key;
pub mod stats;
pub mod waveform;

use std::path::Path;

use djk_core::Key;

pub use decode::{DecodeError, Decoded, SourceInfo, decode_file, is_supported, probe_file};
pub use stats::FileStats;
pub use waveform::Waveform;

/// Everything computed for a track in one pass.
#[derive(Debug, Clone)]
pub struct TrackAnalysis {
    pub stats: FileStats,
    pub bpm: Option<f32>,
    pub key: Option<Key>,
    pub waveform: Waveform,
}

/// Decode a file once and compute stats, tempo, key and waveform.
pub fn analyse_file(path: &Path) -> Result<TrackAnalysis, DecodeError> {
    let decoded = decode_file(path)?;
    Ok(analyse_decoded(path, &decoded))
}

/// Analyse an already-decoded track.
pub fn analyse_decoded(path: &Path, decoded: &Decoded) -> TrackAnalysis {
    let buf = &decoded.buffer;
    let stats = stats::file_stats(path, &decoded.info, buf.duration_secs());
    TrackAnalysis {
        stats,
        bpm: bpm::detect_bpm(&buf.samples, buf.sample_rate),
        key: key::detect_key(&buf.samples, buf.sample_rate),
        waveform: Waveform::from_audio(&buf.samples, buf.sample_rate),
    }
}
