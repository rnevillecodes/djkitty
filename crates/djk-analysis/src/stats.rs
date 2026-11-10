//! File stats: duration, format, sample rate, channels and bitrate.

use std::path::Path;

use lofty::prelude::*;

use crate::decode::SourceInfo;

/// What the details view shows about a file.
#[derive(Debug, Clone, PartialEq)]
pub struct FileStats {
    pub duration_secs: f64,
    /// Short label, e.g. `MP3`, `FLAC`, `WAV`, `AIFF`, `AAC`, `ALAC`.
    pub format: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Average bitrate in kbps; `None` for lossless formats.
    pub bitrate_kbps: Option<u32>,
    pub lossless: bool,
}

/// Human-readable format label from symphonia's container and codec names.
pub fn format_label(info: &SourceInfo) -> String {
    let codec = info.codec.to_ascii_lowercase();
    let container = info.container.to_ascii_lowercase();
    let label = if codec.contains("mp3") || codec.contains("mpa") {
        "MP3"
    } else if codec.contains("flac") {
        "FLAC"
    } else if codec.contains("alac") {
        "ALAC"
    } else if codec.contains("aac") {
        "AAC"
    } else if codec.starts_with("pcm") || codec.contains("pcm") {
        if container.contains("aif") {
            "AIFF"
        } else {
            "WAV"
        }
    } else if container.contains("wav") || container.contains("riff") {
        "WAV"
    } else {
        return info.codec.to_ascii_uppercase();
    };
    label.to_owned()
}

fn is_lossless(label: &str) -> bool {
    matches!(label, "FLAC" | "ALAC" | "WAV" | "AIFF")
}

/// Compute file stats from decode results. The bitrate comes from the codec header when
/// available (lofty), otherwise from file size over duration (covers VBR).
pub fn file_stats(path: &Path, info: &SourceInfo, duration_secs: f64) -> FileStats {
    let format = format_label(info);
    let lossless = is_lossless(&format);
    let bitrate_kbps = if lossless {
        None
    } else {
        lofty::read_from_path(path)
            .ok()
            .and_then(|f| f.properties().audio_bitrate())
            .filter(|&b| b > 0)
            .or_else(|| {
                let size = std::fs::metadata(path).ok()?.len() as f64;
                (duration_secs > 0.0).then(|| (size * 8.0 / duration_secs / 1000.0).round() as u32)
            })
    };
    FileStats {
        duration_secs,
        format,
        sample_rate: info.sample_rate,
        channels: info.channels,
        bitrate_kbps,
        lossless,
    }
}
