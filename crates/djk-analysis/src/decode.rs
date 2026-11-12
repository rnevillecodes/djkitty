//! Full-track decoding to interleaved stereo `f32` with symphonia.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use djk_core::TrackBuffer;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// File extensions djkitty imports (lower case, without the dot).
pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "aif", "aiff", "m4a", "aac"];

/// Whether a path has a supported audio extension.
pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("cannot open file: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported or unrecognised format: {0}")]
    Unsupported(String),
    #[error("file contains no audio track")]
    NoAudioTrack,
    #[error("decoding failed: {0}")]
    Decode(String),
    #[error("file decoded to no audio")]
    Empty,
}

/// Facts about the source file, gathered while decoding.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceInfo {
    /// Container format, e.g. `mp3`, `flac`, `wave`, `aiff`, `isomp4`.
    pub container: String,
    /// Codec short name, e.g. `mp3`, `flac`, `pcm_s16le`, `aac`, `alac`.
    pub codec: String,
    pub sample_rate: u32,
    /// Channel count in the source (before the stereo mixdown).
    pub channels: u16,
    pub bits_per_sample: Option<u32>,
}

/// A decoded track plus source facts.
#[derive(Debug, Clone)]
pub struct Decoded {
    pub buffer: TrackBuffer,
    pub info: SourceInfo,
}

/// Decode a whole file to interleaved stereo `f32` at its native sample rate.
/// Mono is duplicated to both channels; more than two channels are downmixed.
pub fn decode_file(path: &Path) -> Result<Decoded, DecodeError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
    let container = format.format_info().short_name.to_owned();

    let track = format
        .default_track(TrackType::Audio)
        .ok_or(DecodeError::NoAudioTrack)?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(DecodeError::NoAudioTrack)?
        .clone();

    let codecs = symphonia::default::get_codecs();
    let codec = codecs
        .get_audio_decoder(params.codec)
        .map(|r| r.codec.info.short_name.to_owned())
        .unwrap_or_else(|| "unknown".to_owned());
    let mut decoder = codecs
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| DecodeError::Unsupported(e.to_string()))?;

    let mut sample_rate = params.sample_rate.unwrap_or(0);
    let mut src_channels = params.channels.as_ref().map_or(0, |c| c.count());
    let mut out: Vec<f32> = Vec::new();
    let mut block: Vec<f32> = Vec::new();
    let mut decode_errors = 0usize;
    let mut decoded_packets = 0usize;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => {
                if decoded_packets == 0 {
                    return Err(DecodeError::Decode(e.to_string()));
                }
                break;
            }
        };
        if packet.track_id != track_id {
            continue;
        }
        let buf = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(SymError::DecodeError(_)) => {
                decode_errors += 1;
                if decode_errors > 64 && decoded_packets == 0 {
                    return Err(DecodeError::Decode("too many corrupt packets".into()));
                }
                continue;
            }
            Err(e) => return Err(DecodeError::Decode(e.to_string())),
        };
        decoded_packets += 1;
        let spec = buf.spec();
        sample_rate = spec.rate();
        let ch = spec.channels().count().max(1);
        src_channels = ch;
        buf.copy_to_vec_interleaved(&mut block);
        append_stereo(&mut out, &block, ch);
    }

    if out.is_empty() || sample_rate == 0 {
        return Err(DecodeError::Empty);
    }
    out.shrink_to_fit();
    Ok(Decoded {
        buffer: TrackBuffer {
            samples: Arc::from(out),
            sample_rate,
        },
        info: SourceInfo {
            container,
            codec,
            sample_rate,
            channels: src_channels as u16,
            bits_per_sample: params.bits_per_sample,
        },
    })
}

/// Quickly check that a file can be decoded: probe the format and decode the first
/// audio packet. Much cheaper than [`decode_file`].
pub fn probe_file(path: &Path) -> Result<(), DecodeError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(DecodeError::NoAudioTrack)?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(DecodeError::NoAudioTrack)?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
    for _ in 0..32 {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => return Err(DecodeError::Empty),
            Err(e) => return Err(DecodeError::Decode(e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        if decoder.decode(&packet).is_ok() {
            return Ok(());
        }
    }
    Err(DecodeError::Decode("no decodable packets".into()))
}

/// Append interleaved `ch`-channel samples to `out` as interleaved stereo.
fn append_stereo(out: &mut Vec<f32>, block: &[f32], ch: usize) {
    match ch {
        1 => {
            out.reserve(block.len() * 2);
            for &s in block {
                out.push(s);
                out.push(s);
            }
        }
        2 => out.extend_from_slice(block),
        _ => {
            // Downmix: odd-indexed channels to the right, even to the left, scaled so a
            // signal present on all channels keeps its level.
            let frames = block.len() / ch;
            out.reserve(frames * 2);
            let left_n = ch.div_ceil(2) as f32;
            let right_n = (ch / 2) as f32;
            for f in block.chunks_exact(ch) {
                let (mut l, mut r) = (0.0, 0.0);
                for (i, &s) in f.iter().enumerate() {
                    if i % 2 == 0 {
                        l += s;
                    } else {
                        r += s;
                    }
                }
                out.push(l / left_n);
                out.push(r / right_n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_is_duplicated() {
        let mut out = Vec::new();
        append_stereo(&mut out, &[0.1, 0.2], 1);
        assert_eq!(out, vec![0.1, 0.1, 0.2, 0.2]);
    }

    #[test]
    fn multichannel_is_downmixed() {
        let mut out = Vec::new();
        append_stereo(&mut out, &[0.5, 0.5, 0.5, 0.5, 0.5, 0.5], 6);
        assert_eq!(out, vec![0.5, 0.5]);
    }

    #[test]
    fn extension_filter() {
        assert!(is_supported(Path::new("a/b/Track.MP3")));
        assert!(is_supported(Path::new("x.aiff")));
        assert!(!is_supported(Path::new("cover.jpg")));
        assert!(!is_supported(Path::new("notes.txt")));
        assert!(!is_supported(Path::new("noext")));
    }
}
