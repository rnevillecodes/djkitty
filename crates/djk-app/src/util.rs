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
}
