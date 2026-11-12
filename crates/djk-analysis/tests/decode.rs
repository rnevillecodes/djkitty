//! Decoding tests on small fixture files (see `fixtures/make_fixtures.sh`).

use std::path::PathBuf;

use djk_analysis::{DecodeError, decode_file};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Rising crossings with hysteresis (fixtures peak at about 0.09).
fn zero_crossings_left(samples: &[f32]) -> usize {
    let mut armed = false;
    let mut count = 0;
    for f in samples.as_chunks::<2>().0 {
        if f[0] < -0.02 {
            armed = true;
        } else if armed && f[0] > 0.02 {
            armed = false;
            count += 1;
        }
    }
    count
}

#[test]
fn decodes_every_supported_format() {
    for (name, container) in [
        ("tone.mp3", "mp3"),
        ("tone.flac", "flac"),
        ("tone.wav", "wave"),
        ("tone.aiff", "aiff"),
        ("tone.m4a", "isomp4"),
    ] {
        let d = decode_file(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(d.buffer.sample_rate, 44_100, "{name}");
        assert_eq!(d.info.channels, 2, "{name}");
        assert!(
            d.info.container.to_ascii_lowercase().contains(container),
            "{name}: container {}",
            d.info.container
        );
        let secs = d.buffer.duration_secs();
        // Lossy codecs add encoder delay/padding.
        assert!((0.95..1.1).contains(&secs), "{name}: {secs} s");
        // 440 Hz survives decoding (measured over the middle 0.5 s, away from codec
        // priming and padding).
        let mid = &d.buffer.samples[11_025 * 2..33_075 * 2];
        let hz = zero_crossings_left(mid) as f64 / 0.5;
        assert!((436.0..=444.0).contains(&hz), "{name}: {hz} Hz");
        let peak = d.buffer.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.05 && peak <= 1.01, "{name}: peak {peak}");
    }
}

#[test]
fn mono_is_duplicated_to_stereo() {
    let d = decode_file(&fixture("mono.wav")).unwrap();
    assert_eq!(d.info.channels, 1);
    assert_eq!(d.buffer.frames(), 44_100);
    assert!(
        d.buffer
            .samples
            .as_chunks::<2>()
            .0
            .iter()
            .all(|f| f[0] == f[1])
    );
}

#[test]
fn corrupt_file_is_a_typed_error() {
    let err = decode_file(&fixture("corrupt.mp3")).unwrap_err();
    assert!(
        matches!(
            err,
            DecodeError::Unsupported(_) | DecodeError::Decode(_) | DecodeError::Empty
        ),
        "{err:?}"
    );
}

#[test]
fn missing_file_is_io_error() {
    let err = decode_file(&fixture("does-not-exist.mp3")).unwrap_err();
    assert!(matches!(err, DecodeError::Io(_)));
}
