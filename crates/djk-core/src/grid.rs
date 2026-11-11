//! Beatgrids: where the beats and bars of a track fall.
//!
//! A grid is constant-tempo and starts at its first beat (`start_secs`), normally where
//! the music becomes audible: there are no beats before it. Beats follow every
//! `60 / bpm` seconds, and `first_downbeat` says which of them (0–3) starts bar 1, so
//! bars can be re-phrased without moving where the grid starts. Times are in *track*
//! seconds, so pitch and tempo changes don't move the grid. Grids are a visual aid for
//! learning to beatmatch by ear and eye; djkitty never syncs decks automatically.

/// Beats per bar (4/4 time).
pub const BEATS_PER_BAR: i64 = 4;
/// Bars per phrase, for overview markers.
pub const BARS_PER_PHRASE: i64 = 16;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Beatgrid {
    pub bpm: f64,
    /// Time of the grid's first beat (beat 0), in track seconds.
    pub start_secs: f64,
    /// Which beat (0–3, counted from the first) is beat 1 of bar 1. Earlier beats are a
    /// pickup (bar 0).
    pub first_downbeat: i64,
}

/// A position on the grid. Bar 1 starts at the first downbeat; pickup beats before it
/// are in bar 0.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BarBeat {
    pub bar: i64,
    /// 1-based beat within the bar (1..=4).
    pub beat: i64,
}

impl std::fmt::Display for BarBeat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.bar, self.beat)
    }
}

impl Beatgrid {
    /// A grid whose first beat, at `start_secs`, is beat 1 of bar 1.
    pub fn new(bpm: f64, start_secs: f64) -> Self {
        Self::with_phase(bpm, start_secs, 0)
    }

    pub fn with_phase(bpm: f64, start_secs: f64, first_downbeat: i64) -> Self {
        Self {
            bpm,
            start_secs,
            first_downbeat: first_downbeat.rem_euclid(BEATS_PER_BAR),
        }
    }

    /// Seconds per beat.
    pub fn period(&self) -> f64 {
        60.0 / self.bpm.max(1e-6)
    }

    /// Fractional beat index at `t` (beat 0 is the first beat).
    pub fn beat_at(&self, t: f64) -> f64 {
        (t - self.start_secs) / self.period()
    }

    /// Time of beat `n` (beat 0 is the first beat).
    pub fn beat_time(&self, n: i64) -> f64 {
        self.start_secs + n as f64 * self.period()
    }

    /// Indices of the grid's beats within `[t0, t1]` (none before the first beat).
    pub fn beats_between(&self, t0: f64, t1: f64) -> std::ops::RangeInclusive<i64> {
        (self.beat_at(t0).ceil() as i64).max(0)..=self.beat_at(t1).floor() as i64
    }

    /// Whether beat `n` starts a bar.
    pub fn is_downbeat(&self, n: i64) -> bool {
        (n - self.first_downbeat).rem_euclid(BEATS_PER_BAR) == 0
    }

    /// The bar beat `n` belongs to (1 from the first downbeat; 0 for a pickup).
    pub fn bar_of(&self, n: i64) -> i64 {
        (n - self.first_downbeat).div_euclid(BEATS_PER_BAR) + 1
    }

    /// Time of the downbeat starting `bar` (bar 1 is the first downbeat).
    pub fn bar_time(&self, bar: i64) -> f64 {
        self.beat_time(self.first_downbeat + (bar - 1) * BEATS_PER_BAR)
    }

    /// The bar and beat being played at `t`, or `None` before the first beat.
    pub fn bar_beat(&self, t: f64) -> Option<BarBeat> {
        let n = self.beat_at(t).floor() as i64;
        (n >= 0).then(|| BarBeat {
            bar: self.bar_of(n),
            beat: (n - self.first_downbeat).rem_euclid(BEATS_PER_BAR) + 1,
        })
    }

    /// Make the beat nearest `t` beat 1 of a bar, moved exactly onto `t`. Bars are
    /// re-phrased from there; the grid still starts at (about) the same place, extended
    /// earlier if `t` is before its first beat.
    pub fn with_downbeat_at(self, t: f64) -> Self {
        let n = self.beat_at(t).round() as i64;
        if n < 0 {
            return Self::with_phase(self.bpm, t, 0);
        }
        Self::with_phase(self.bpm, t - n as f64 * self.period(), n)
    }

    /// Shift the whole grid later by `secs` (earlier if negative).
    pub fn nudged(self, secs: f64) -> Self {
        Self {
            start_secs: self.start_secs + secs,
            ..self
        }
    }

    /// Change the tempo while keeping the downbeat nearest `pivot_secs` where it is (so
    /// the part of the track you are looking at stays lined up) and the grid starting
    /// at about the same place. Halving and doubling keep every bar line.
    pub fn with_bpm(self, bpm: f64, pivot_secs: f64) -> Self {
        let bpm = bpm.clamp(20.0, 400.0);
        // The downbeat nearest the pivot (or bar 1 if the pivot is before the grid).
        let bar = ((pivot_secs - self.bar_time(1)) / (self.period() * BEATS_PER_BAR as f64))
            .round()
            .max(0.0) as i64;
        let k = self.first_downbeat + bar * BEATS_PER_BAR;
        let t_k = self.beat_time(k);
        // The same moment counted in new beats, so the start stays close by.
        let k_new = (k as f64 * bpm / self.bpm).round() as i64;
        let period_new = 60.0 / bpm;
        Self::with_phase(bpm, t_k - k_new as f64 * period_new, k_new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn beats_start_at_the_first_beat() {
        let g = Beatgrid::new(120.0, 0.25);
        assert!(close(g.period(), 0.5));
        assert!(close(g.beat_time(3), 1.75));
        // Nothing before the first beat.
        assert_eq!(g.beats_between(0.0, 1.3).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(g.beats_between(-5.0, 0.2).count(), 0);
    }

    #[test]
    fn bar_beat_counts_from_bar_one() {
        let g = Beatgrid::new(120.0, 0.25);
        assert_eq!(g.bar_beat(0.25), Some(BarBeat { bar: 1, beat: 1 }));
        assert_eq!(g.bar_beat(0.8), Some(BarBeat { bar: 1, beat: 2 }));
        assert_eq!(g.bar_beat(2.3), Some(BarBeat { bar: 2, beat: 1 }));
        assert_eq!(g.bar_beat(0.1), None);
        assert_eq!(g.bar_beat(2.3).unwrap().to_string(), "2.1");
    }

    #[test]
    fn a_pickup_before_the_first_downbeat_is_bar_zero() {
        // Starts on beat 4 of a pickup bar: the second beat is bar 1.
        let g = Beatgrid::with_phase(120.0, 0.0, 1);
        assert_eq!(g.bar_beat(0.1), Some(BarBeat { bar: 0, beat: 4 }));
        assert_eq!(g.bar_beat(0.6), Some(BarBeat { bar: 1, beat: 1 }));
        assert!(g.is_downbeat(1) && !g.is_downbeat(0));
        assert!(close(g.bar_time(1), 0.5));
    }

    #[test]
    fn setting_the_downbeat_rephrases_without_moving_the_start() {
        // Beat 2 (at 1.0 s) should have been the downbeat, and it is really at 1.01 s.
        let g = Beatgrid::new(120.0, 0.0).with_downbeat_at(1.01);
        assert!(close(g.start_secs, 0.01));
        assert_eq!(g.bar_beat(1.02), Some(BarBeat { bar: 1, beat: 1 }));
        assert_eq!(g.bar_beat(0.02), Some(BarBeat { bar: 0, beat: 3 }));
        // Before the grid: it is extended back to there.
        let h = Beatgrid::new(120.0, 2.0).with_downbeat_at(0.4);
        assert!(close(h.start_secs, 0.4));
        assert_eq!(h.bar_beat(0.4), Some(BarBeat { bar: 1, beat: 1 }));
    }

    #[test]
    fn nudging_shifts_every_beat() {
        let g = Beatgrid::new(120.0, 0.25).nudged(0.01);
        assert!(close(g.beat_time(4), 2.26));
    }

    #[test]
    fn tempo_changes_keep_the_nearby_downbeat_and_the_start() {
        let g = Beatgrid::new(120.0, 1.0);
        // Bar 30 starts at 1 + 29 × 2 = 59 s. Doubling keeps it and every bar line, and
        // keeps the start.
        let d = g.with_bpm(240.0, 59.4);
        assert!(close(d.bpm, 240.0));
        assert!(close(d.start_secs, 1.0));
        assert_eq!(d.bar_beat(59.0).map(|b| b.beat), Some(1));
        // Bars are half as long now, so the old bar 30 is bar 59.
        assert!(close(d.bar_time(59), 59.0));
        // Halving likewise keeps the start.
        let h = g.with_bpm(60.0, 59.4);
        assert!(close(h.start_secs, 1.0));
        // A fine change keeps the bar at the pivot exactly and the start within a few ms.
        let f = g.with_bpm(120.05, 59.4);
        let n = f.beat_at(59.0);
        assert!(close(n, n.round()) && f.is_downbeat(n.round() as i64));
        assert!((f.start_secs - 1.0).abs() < 0.03, "{}", f.start_secs);
    }
}
