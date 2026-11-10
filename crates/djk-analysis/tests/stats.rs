//! File stats on fixture files.

use std::path::PathBuf;

use djk_analysis::analyse_file;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn stats_for_each_format() {
    for (name, format, lossless, kbps) in [
        ("tone.mp3", "MP3", false, Some(128)),
        ("tone.m4a", "AAC", false, None),
        ("tone.flac", "FLAC", true, None),
        ("tone.wav", "WAV", true, None),
        ("tone.aiff", "AIFF", true, None),
    ] {
        let a = analyse_file(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let s = &a.stats;
        assert_eq!(s.format, format, "{name}");
        assert_eq!(s.lossless, lossless, "{name}");
        assert_eq!(s.sample_rate, 44_100, "{name}");
        assert_eq!(s.channels, 2, "{name}");
        assert!((0.95..1.1).contains(&s.duration_secs), "{name}");
        if lossless {
            assert_eq!(s.bitrate_kbps, None, "{name}");
        } else {
            let b = s
                .bitrate_kbps
                .unwrap_or_else(|| panic!("{name}: no bitrate"));
            if let Some(k) = kbps {
                // Within 3%: on a 1 s file, encoder padding skews size/duration slightly.
                assert!(b.abs_diff(k) * 100 <= k * 3, "{name}: {b} kbps");
            } else {
                assert!((32..=320).contains(&b), "{name}: {b} kbps");
            }
        }
        // Waveform covers the duration at 150 bins/s.
        let expected = (s.duration_secs * 150.0).ceil() as usize;
        assert!(a.waveform.bins.len().abs_diff(expected) <= 1, "{name}");
    }
}
