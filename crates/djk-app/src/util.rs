//! Small formatting helpers.

/// Format seconds as `m:ss` (or `h:mm:ss` from one hour).
pub fn fmt_time(secs: f64) -> String {
    let total = secs.max(0.0).floor() as u64;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Format a byte count for people: `850 KB`, `12.4 MB`, `1.2 GB` (decimal units, as
/// Windows Explorer's sizes are close enough to read alongside).
pub fn fmt_bytes(bytes: u64) -> String {
    let b = bytes as f64;
    if b < 1e6 {
        format!("{:.0} KB", (b / 1e3).ceil())
    } else if b < 1e9 {
        format!("{:.1} MB", b / 1e6)
    } else {
        format!("{:.1} GB", b / 1e9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_times() {
        assert_eq!(fmt_time(90.0), "1:30");
        assert_eq!(fmt_time(270.4), "4:30");
        assert_eq!(fmt_time(5.0), "0:05");
        assert_eq!(fmt_time(3725.0), "1:02:05");
        assert_eq!(fmt_time(-3.0), "0:00");
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(fmt_bytes(0), "0 KB");
        assert_eq!(fmt_bytes(850_000), "850 KB");
        assert_eq!(fmt_bytes(12_400_000), "12.4 MB");
        assert_eq!(fmt_bytes(1_234_000_000), "1.2 GB");
    }
}
