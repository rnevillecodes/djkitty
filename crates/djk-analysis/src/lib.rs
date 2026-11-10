//! Decoding, file stats, BPM, key and waveform analysis.

pub mod decode;

pub use decode::{DecodeError, Decoded, SourceInfo, decode_file, is_supported};
