//! A single deck: a fractional read head over a decoded track buffer.
//!
//! Everything here runs on the audio thread: no allocation, locking or I/O.

use djk_core::{DeckState, PITCH_RANGE, TrackBuffer};

/// Pitch-fraction of speed change per jog tick per block while playing (before decay).
pub const JOG_NUDGE_PER_TICK: f64 = 0.01;
/// Largest nudge the jog wheel can apply (±50%).
pub const JOG_NUDGE_MAX: f64 = 0.5;
/// Time constant (seconds) for the jog nudge to fade back to zero once the wheel stops.
pub const JOG_NUDGE_DECAY_SECS: f64 = 0.06;
/// Seconds of audio moved per jog tick while paused.
pub const JOG_SEEK_SECS_PER_TICK: f64 = 0.004;

/// One deck of the engine.
#[derive(Default)]
pub struct Deck {
    buffer: Option<TrackBuffer>,
    token: u64,
    position: f64,
    playing: bool,
    previewing: bool,
    cue: f64,
    pitch: f32,
    /// Accumulated jog velocity (ticks), decays towards 0.
    jog_velocity: f64,
    nudge: f64,
}

impl Deck {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the deck's track, returning the old buffer so the caller can retire it
    /// off the audio thread. Resets position and cue to the start.
    pub fn load(&mut self, buffer: TrackBuffer, token: u64) -> Option<TrackBuffer> {
        let old = self.buffer.replace(buffer);
        self.token = token;
        self.position = 0.0;
        self.cue = 0.0;
        self.playing = false;
        self.previewing = false;
        self.jog_velocity = 0.0;
        self.nudge = 0.0;
        old
    }

    pub fn is_loaded(&self) -> bool {
        self.buffer.is_some()
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    fn frames(&self) -> f64 {
        self.buffer.as_ref().map_or(0.0, |b| b.frames() as f64)
    }

    fn track_rate(&self) -> f64 {
        self.buffer
            .as_ref()
            .map_or(44_100.0, |b| f64::from(b.sample_rate))
    }

    fn at_end(&self) -> bool {
        self.position >= self.frames()
    }

    fn at_cue(&self) -> bool {
        (self.position - self.cue).abs() <= self.track_rate() / 1000.0
    }

    pub fn play(&mut self) {
        if self.is_loaded() && !self.at_end() {
            self.playing = true;
        }
        // Pressing play during a cue preview keeps playing after cue is released.
        self.previewing = false;
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.previewing = false;
    }

    pub fn toggle_play(&mut self) {
        if self.playing && !self.previewing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// CDJ-style cue press.
    ///
    /// - Playing: return to the cue point and pause.
    /// - Paused away from the cue point: set the cue point here, then preview.
    /// - Paused at the cue point: preview (play while held).
    pub fn cue_press(&mut self) {
        if !self.is_loaded() {
            return;
        }
        if self.playing && !self.previewing {
            self.position = self.cue;
            self.playing = false;
            return;
        }
        if !self.at_cue() {
            self.cue = self.position;
        }
        if !self.at_end() {
            self.playing = true;
            self.previewing = true;
        }
    }

    /// CDJ-style cue release: a preview returns to the cue point and pauses.
    pub fn cue_release(&mut self) {
        if self.previewing {
            self.previewing = false;
            self.playing = false;
            self.position = self.cue;
        }
    }

    pub fn seek(&mut self, frame: f64) {
        self.position = frame.clamp(0.0, self.frames());
    }

    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch.clamp(-PITCH_RANGE, PITCH_RANGE);
    }

    /// Jog wheel: nudge speed while playing, move the playhead while paused.
    pub fn jog(&mut self, ticks: f32) {
        if !self.is_loaded() {
            return;
        }
        if self.playing {
            self.jog_velocity += f64::from(ticks);
        } else {
            let frames = f64::from(ticks) * JOG_SEEK_SECS_PER_TICK * self.track_rate();
            self.seek(self.position + frames);
        }
    }

    /// Current speed relative to the track's own speed (0 when paused).
    pub fn speed(&self) -> f64 {
        if self.playing {
            (1.0 + f64::from(self.pitch)) * (1.0 + self.nudge)
        } else {
            0.0
        }
    }

    /// Render `out_l.len()` frames, adding nothing (writes, not mixes). Silence when paused.
    pub fn render(&mut self, device_rate: u32, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len().min(out_r.len());
        let block_secs = n as f64 / f64::from(device_rate.max(1));
        self.update_nudge(block_secs);

        let Some(buffer) = self.buffer.as_ref().filter(|_| self.playing) else {
            out_l[..n].fill(0.0);
            out_r[..n].fill(0.0);
            return;
        };
        let samples = &buffer.samples[..];
        let frames = buffer.frames();
        let end = frames as f64;
        let step = (f64::from(buffer.sample_rate) / f64::from(device_rate.max(1)))
            * (1.0 + f64::from(self.pitch))
            * (1.0 + self.nudge);

        let mut pos = self.position;
        let mut i = 0;
        while i < n {
            if pos >= end {
                break;
            }
            let (l, r) = hermite_frame(samples, frames, pos);
            out_l[i] = l;
            out_r[i] = r;
            pos += step;
            i += 1;
        }
        out_l[i..n].fill(0.0);
        out_r[i..n].fill(0.0);

        if pos >= end {
            pos = end;
            self.playing = false;
            self.previewing = false;
        }
        self.position = pos;
    }

    fn update_nudge(&mut self, block_secs: f64) {
        if !self.playing {
            self.jog_velocity = 0.0;
            self.nudge = 0.0;
            return;
        }
        self.nudge = (self.jog_velocity * JOG_NUDGE_PER_TICK).clamp(-JOG_NUDGE_MAX, JOG_NUDGE_MAX);
        self.jog_velocity *= (-block_secs / JOG_NUDGE_DECAY_SECS).exp();
        if self.jog_velocity.abs() < 1e-3 {
            self.jog_velocity = 0.0;
        }
    }

    pub fn state(&self) -> DeckState {
        DeckState {
            loaded: self.is_loaded(),
            track_token: self.token,
            track_frames: self.frames() as u64,
            track_rate: self.track_rate() as u32,
            position: self.position,
            playing: self.playing,
            previewing: self.previewing,
            cue: self.cue,
            pitch: self.pitch,
            speed: self.speed(),
        }
    }
}

/// 4-point, 3rd-order Hermite interpolation of an interleaved stereo frame at `pos`.
#[inline]
fn hermite_frame(samples: &[f32], frames: usize, pos: f64) -> (f32, f32) {
    let i = pos.floor() as isize;
    let t = (pos - i as f64) as f32;
    let last = frames as isize - 1;
    let idx = |k: isize| (i + k).clamp(0, last) as usize * 2;
    let (im1, i0, i1, i2) = (idx(-1), idx(0), idx(1), idx(2));
    let l = hermite(samples[im1], samples[i0], samples[i1], samples[i2], t);
    let r = hermite(
        samples[im1 + 1],
        samples[i0 + 1],
        samples[i1 + 1],
        samples[i2 + 1],
        t,
    );
    (l, r)
}

#[inline]
fn hermite(ym1: f32, y0: f32, y1: f32, y2: f32, t: f32) -> f32 {
    let c0 = y0;
    let c1 = 0.5 * (y1 - ym1);
    let c2 = ym1 - 2.5 * y0 + 2.0 * y1 - 0.5 * y2;
    let c3 = 0.5 * (y2 - ym1) + 1.5 * (y0 - y1);
    ((c3 * t + c2) * t + c1) * t + c0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn sine(freq: f32, rate: u32, secs: f32) -> TrackBuffer {
        let n = (rate as f32 * secs) as usize;
        let mut s = Vec::with_capacity(n * 2);
        for i in 0..n {
            let v = (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin() * 0.5;
            s.push(v);
            s.push(v);
        }
        TrackBuffer {
            samples: Arc::from(s),
            sample_rate: rate,
        }
    }

    fn render_all(deck: &mut Deck, rate: u32, max_frames: usize) -> Vec<f32> {
        let mut out = Vec::new();
        let mut l = vec![0.0; 256];
        let mut r = vec![0.0; 256];
        while out.len() < max_frames && deck.is_playing() {
            deck.render(rate, &mut l, &mut r);
            out.extend_from_slice(&l);
        }
        out
    }

    fn zero_crossings(x: &[f32]) -> usize {
        x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
    }

    #[test]
    fn resampling_keeps_frequency() {
        let mut deck = Deck::new();
        deck.load(sine(1000.0, 44_100, 1.0), 1);
        deck.play();
        let out = render_all(&mut deck, 48_000, 48_000);
        // 1 s of output at 48 kHz should contain ~1000 cycles.
        let cycles = zero_crossings(&out[..48_000.min(out.len())]);
        assert!((995..=1001).contains(&cycles), "cycles = {cycles}");
    }

    #[test]
    fn pitch_shortens_playback() {
        let rate = 48_000;
        let mut deck = Deck::new();
        deck.load(sine(440.0, rate, 2.0), 1);
        deck.set_pitch(0.024);
        deck.play();
        let mut l = vec![0.0; 1];
        let mut r = vec![0.0; 1];
        let mut rendered = 0usize;
        while deck.is_playing() {
            deck.render(rate, &mut l, &mut r);
            rendered += 1;
        }
        let expected = 96_000.0 / 1.024;
        assert!(
            (rendered as f64 - expected).abs() < 2.0,
            "rendered {rendered}, expected {expected}"
        );
    }

    #[test]
    fn stops_at_end() {
        let mut deck = Deck::new();
        deck.load(sine(440.0, 48_000, 0.1), 1);
        deck.play();
        let out = render_all(&mut deck, 48_000, 1_000_000);
        assert!(!deck.is_playing());
        assert_eq!(deck.state().position, 4_800.0);
        assert!(out.len() < 6_000);
        // Play at the end does nothing.
        deck.play();
        assert!(!deck.is_playing());
    }

    fn loaded_deck() -> Deck {
        let mut deck = Deck::new();
        deck.load(sine(440.0, 1_000, 200.0), 1);
        deck
    }

    #[test]
    fn cue_sets_point_when_paused() {
        let mut deck = loaded_deck();
        deck.seek(32_000.0); // 0:32
        deck.cue_press();
        assert_eq!(deck.state().cue, 32_000.0);
        deck.cue_release();
        assert_eq!(deck.state().position, 32_000.0);
        assert!(!deck.is_playing());
    }

    #[test]
    fn cue_returns_and_pauses_when_playing() {
        let mut deck = loaded_deck();
        deck.seek(32_000.0);
        deck.cue_press();
        deck.cue_release();
        deck.seek(70_000.0); // 1:10
        deck.play();
        deck.cue_press();
        let s = deck.state();
        assert_eq!(s.position, 32_000.0);
        assert!(!s.playing);
        deck.cue_release();
        assert!(!deck.is_playing());
        assert_eq!(deck.state().position, 32_000.0);
    }

    #[test]
    fn cue_preview_plays_while_held() {
        let mut deck = loaded_deck();
        deck.seek(32_000.0);
        deck.cue_press();
        deck.cue_release();
        // Now paused at the cue point: hold cue.
        deck.cue_press();
        assert!(deck.is_playing());
        let mut l = vec![0.0; 500];
        let mut r = vec![0.0; 500];
        deck.render(1_000, &mut l, &mut r);
        assert!(deck.state().position > 32_000.0);
        assert!(l.iter().any(|&v| v != 0.0));
        deck.cue_release();
        let s = deck.state();
        assert!(!s.playing);
        assert_eq!(s.position, 32_000.0);
    }

    #[test]
    fn play_during_preview_keeps_playing() {
        let mut deck = loaded_deck();
        deck.cue_press(); // at cue (0) → preview
        deck.play();
        deck.cue_release();
        assert!(deck.is_playing());
    }

    #[test]
    fn jog_nudges_while_playing_and_recovers() {
        let rate = 1_000;
        let mut deck = loaded_deck();
        deck.play();
        let mut l = vec![0.0; 10];
        let mut r = vec![0.0; 10];
        deck.jog(5.0);
        deck.render(rate, &mut l, &mut r);
        assert!(deck.speed() > 1.0, "speed {}", deck.speed());
        for _ in 0..200 {
            deck.render(rate, &mut l, &mut r);
        }
        assert!((deck.speed() - 1.0).abs() < 1e-6, "speed {}", deck.speed());
    }

    #[test]
    fn jog_seeks_backwards_while_paused() {
        let mut deck = loaded_deck();
        deck.seek(10_000.0);
        deck.jog(-10.0);
        let p = deck.state().position;
        assert!(p < 10_000.0);
        assert!(!deck.is_playing());
    }

    #[test]
    fn pitch_is_clamped() {
        let mut deck = loaded_deck();
        deck.set_pitch(0.5);
        assert_eq!(deck.state().pitch, PITCH_RANGE);
    }
}
