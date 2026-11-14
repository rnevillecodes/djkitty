//! A single deck: a fractional read head over a decoded track buffer.
//!
//! Everything here runs on the audio thread: no allocation, locking or I/O.

use djk_core::{ActiveLoop, DeckState, HotCues, PITCH_RANGE, TrackBuffer};

/// Pitch-fraction of speed change per jog tick per block while playing (before decay).
pub const JOG_NUDGE_PER_TICK: f64 = 0.01;
/// Largest nudge the jog wheel can apply (±50%).
pub const JOG_NUDGE_MAX: f64 = 0.5;
/// Time constant (seconds) for the jog nudge to fade back to zero once the wheel stops.
pub const JOG_NUDGE_DECAY_SECS: f64 = 0.06;
/// Seconds of audio moved per jog tick while paused.
pub const JOG_SEEK_SECS_PER_TICK: f64 = 0.004;
/// Length of the crossfade at each loop wrap, which hides the jump's click.
pub const LOOP_XFADE_SECS: f64 = 0.003;

/// One deck of the engine.
#[derive(Default)]
pub struct Deck {
    buffer: Option<TrackBuffer>,
    token: u64,
    position: f64,
    playing: bool,
    /// Playing only while cue or a hot cue is held (see `held_hot_cue`).
    previewing: bool,
    cue: f64,
    pitch: f32,
    /// Accumulated jog velocity (ticks), decays towards 0.
    jog_velocity: f64,
    nudge: f64,
    hot_cues: HotCues,
    /// The hot cue being held for a preview, and where releasing it returns to.
    held_hot_cue: Option<(u8, f64)>,
    active_loop: Option<ActiveLoop>,
    beat_frames: Option<f64>,
    loop_refusals: u32,
    load_refusals: u32,
    /// After a loop wrap: the read position carrying on past the loop end, faded out
    /// over `xfade_left` more output frames.
    xfade_from: f64,
    xfade_left: u32,
}

impl Deck {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the deck's track, returning the old buffer so the caller can retire it
    /// off the audio thread. Resets position and cue to the start, ends any loop and
    /// takes the new track's saved hot cues.
    pub fn load(
        &mut self,
        buffer: TrackBuffer,
        token: u64,
        hot_cues: HotCues,
    ) -> Option<TrackBuffer> {
        let old = self.buffer.replace(buffer);
        self.token = token;
        self.position = 0.0;
        self.cue = 0.0;
        self.playing = false;
        self.previewing = false;
        self.jog_velocity = 0.0;
        self.nudge = 0.0;
        let frames = self.frames();
        // Up to and including the end, where `hot_cue_press` can set one.
        self.hot_cues = hot_cues.map(|c| c.filter(|&p| (0.0..=frames).contains(&p)));
        self.held_hot_cue = None;
        self.active_loop = None;
        self.beat_frames = None;
        self.xfade_left = 0;
        old
    }

    /// Empty the deck, returning its buffer so the caller can retire it off the audio
    /// thread. Refused (returns `None`, nothing changes) while playing.
    pub fn eject(&mut self) -> Option<TrackBuffer> {
        if self.playing {
            return None;
        }
        let old = self.buffer.take()?;
        self.token = 0;
        self.position = 0.0;
        self.cue = 0.0;
        self.previewing = false;
        self.jog_velocity = 0.0;
        self.nudge = 0.0;
        self.hot_cues = Default::default();
        self.held_hot_cue = None;
        self.active_loop = None;
        self.beat_frames = None;
        self.xfade_left = 0;
        Some(old)
    }

    /// Move the playhead for a DJ action (seek, cue, hot cue). Leaving the active loop's
    /// region ends the loop.
    fn move_to(&mut self, frame: f64) {
        if frame.is_nan() {
            return;
        }
        self.position = frame.clamp(0.0, self.frames());
        self.xfade_left = 0;
        if let Some(l) = self.active_loop
            && !(l.start..l.end).contains(&self.position)
        {
            self.active_loop = None;
        }
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
        // Pressing play during a cue or hot cue preview keeps playing after release.
        self.previewing = false;
        self.held_hot_cue = None;
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.previewing = false;
        self.held_hot_cue = None;
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
            self.move_to(self.cue);
            self.playing = false;
            return;
        }
        if !self.at_cue() {
            self.cue = self.position;
        }
        if !self.at_end() {
            self.playing = true;
            self.previewing = true;
            self.held_hot_cue = None;
        }
    }

    /// CDJ-style cue release: a cue preview returns to the cue point and pauses.
    pub fn cue_release(&mut self) {
        if self.previewing && self.held_hot_cue.is_none() {
            self.previewing = false;
            self.playing = false;
            self.move_to(self.cue);
        }
    }

    pub fn seek(&mut self, frame: f64) {
        self.move_to(frame);
    }

    /// Hot cue pad pressed. An empty slot is set at the playhead without changing
    /// playback; a set one is jumped to, and on a paused deck it plays while held.
    pub fn hot_cue_press(&mut self, slot: u8) {
        let Some(&cue) = self.hot_cues.get(usize::from(slot)) else {
            return;
        };
        if !self.is_loaded() {
            return;
        }
        let Some(point) = cue else {
            self.hot_cues[usize::from(slot)] = Some(self.position.min(self.frames()));
            return;
        };
        self.move_to(point);
        if self.playing && !self.previewing {
            return; // jump and keep playing
        }
        if self.at_end() {
            return;
        }
        self.playing = true;
        self.previewing = true;
        self.held_hot_cue = Some((slot, point));
    }

    /// Hot cue pad released: a preview from this slot returns to it and pauses.
    pub fn hot_cue_release(&mut self, slot: u8) {
        if let Some((held, point)) = self.held_hot_cue
            && held == slot
        {
            self.held_hot_cue = None;
            self.previewing = false;
            self.playing = false;
            self.move_to(point);
        }
    }

    pub fn clear_hot_cue(&mut self, slot: u8) {
        if let Some(c) = self.hot_cues.get_mut(usize::from(slot)) {
            *c = None;
        }
    }

    /// Empty every hot cue slot if the deck holds the track `token`. Playback is unchanged.
    pub fn clear_hot_cues(&mut self, token: u64) {
        if token == self.token {
            self.hot_cues = [None; djk_core::HOT_CUE_COUNT];
        }
    }

    /// Record a load refused because the deck was playing.
    pub fn note_refused_load(&mut self) {
        self.load_refusals = self.load_refusals.wrapping_add(1);
    }

    /// Set the length of one beat for the track `token` (ignored for any other track).
    pub fn set_beat_length(&mut self, token: u64, frames: Option<f64>) {
        if token == self.token {
            self.beat_frames = frames.filter(|f| f.is_finite() && *f > 0.0);
        }
    }

    /// Beat loop pad: start a loop of `beats` at the playhead (exactly there, never
    /// snapped to the grid), resize the active loop from its start, or exit the loop if
    /// it already has this length. Refused (and counted) when the tempo is unknown.
    pub fn beat_loop(&mut self, beats: f64) {
        if !self.is_loaded() || beats.is_nan() || beats <= 0.0 {
            return;
        }
        let Some(beat) = self.beat_frames else {
            self.loop_refusals = self.loop_refusals.wrapping_add(1);
            return;
        };
        let frames = self.frames();
        let len = beats * beat;
        match self.active_loop {
            Some(l) if l.beats == beats => self.active_loop = None,
            Some(l) => {
                let end = (l.start + len).min(frames);
                if self.position >= end && end > l.start {
                    self.position = l.start + (self.position - l.start) % (end - l.start);
                    self.xfade_left = 0;
                }
                self.active_loop = Some(ActiveLoop { end, beats, ..l });
            }
            None => {
                let start = self.position;
                let end = (start + len).min(frames);
                if end - start >= 1.0 {
                    self.active_loop = Some(ActiveLoop { start, end, beats });
                }
            }
        }
    }

    pub fn exit_loop(&mut self) {
        self.active_loop = None;
    }

    pub fn set_pitch(&mut self, pitch: f32) {
        if pitch.is_nan() {
            return;
        }
        self.pitch = pitch.clamp(-PITCH_RANGE, PITCH_RANGE);
    }

    /// Jog wheel: nudge speed while playing, move the playhead while paused.
    pub fn jog(&mut self, ticks: f32) {
        if !self.is_loaded() || !ticks.is_finite() {
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
    /// Returns false when the deck is not playing, so the block is all zeros.
    pub fn render(&mut self, device_rate: u32, out_l: &mut [f32], out_r: &mut [f32]) -> bool {
        let n = out_l.len().min(out_r.len());
        let block_secs = n as f64 / f64::from(device_rate.max(1));
        self.update_nudge(block_secs);

        let Some(buffer) = self.buffer.as_ref().filter(|_| self.playing) else {
            out_l[..n].fill(0.0);
            out_r[..n].fill(0.0);
            return false;
        };
        let samples = &buffer.samples[..];
        let frames = buffer.frames();
        let end = frames as f64;
        let step = (f64::from(buffer.sample_rate) / f64::from(device_rate.max(1)))
            * (1.0 + f64::from(self.pitch))
            * (1.0 + self.nudge);

        let xfade_total = (f64::from(device_rate) * LOOP_XFADE_SECS).max(1.0) as u32;
        let mut pos = self.position;
        let mut i = 0;
        while i < n {
            if let Some(lp) = self.active_loop
                && pos >= lp.end
            {
                // Wrap, keeping the overshoot so the loop period stays exact, and fade
                // out the audio that would have followed the loop end.
                self.xfade_from = pos;
                self.xfade_left = xfade_total;
                pos = lp.start + (pos - lp.end) % (lp.end - lp.start);
            }
            if pos >= end {
                break;
            }
            let (mut l, mut r) = hermite_frame(samples, frames, pos);
            if self.xfade_left > 0 {
                // Equal-power crossfade from the old read position into the new one.
                let t = 1.0 - self.xfade_left as f32 / xfade_total as f32;
                let (fade_in, fade_out) = (
                    (t * std::f32::consts::FRAC_PI_2).sin(),
                    (t * std::f32::consts::FRAC_PI_2).cos(),
                );
                let (ol, or) = if self.xfade_from < end {
                    hermite_frame(samples, frames, self.xfade_from)
                } else {
                    (0.0, 0.0)
                };
                l = l * fade_in + ol * fade_out;
                r = r * fade_in + or * fade_out;
                self.xfade_from += step;
                self.xfade_left -= 1;
            }
            out_l[i] = l;
            out_r[i] = r;
            pos += step;
            i += 1;
        }
        // Wrap now if the block ended exactly at the loop end, so the playhead never
        // reports a position past it (the crossfade carries on into the next block).
        if let Some(lp) = self.active_loop
            && pos >= lp.end
        {
            self.xfade_from = pos;
            self.xfade_left = xfade_total;
            pos = lp.start + (pos - lp.end) % (lp.end - lp.start);
        }
        out_l[i..n].fill(0.0);
        out_r[i..n].fill(0.0);

        if pos >= end {
            pos = end;
            self.playing = false;
            self.previewing = false;
        }
        self.position = pos;
        true
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
            hot_cues: self.hot_cues,
            active_loop: self.active_loop,
            beat_frames: self.beat_frames,
            loop_refusals: self.loop_refusals,
            load_refusals: self.load_refusals,
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
        deck.load(sine(1000.0, 44_100, 1.0), 1, Default::default());
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
        deck.load(sine(440.0, rate, 2.0), 1, Default::default());
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
        deck.load(sine(440.0, 48_000, 0.1), 1, Default::default());
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
        deck.load(sine(440.0, 1_000, 200.0), 1, Default::default());
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

    // ---- Hot cues

    fn render_frames(deck: &mut Deck, rate: u32, frames: usize) {
        let mut l = vec![0.0; frames];
        let mut r = vec![0.0; frames];
        deck.render(rate, &mut l, &mut r);
    }

    #[test]
    fn empty_hot_cue_is_set_at_the_playhead_while_playing() {
        let mut deck = loaded_deck();
        deck.seek(62_350.0);
        deck.play();
        deck.hot_cue_press(2);
        deck.hot_cue_release(2);
        let s = deck.state();
        assert_eq!(s.hot_cues[2], Some(62_350.0));
        assert!(s.playing, "playback is not interrupted");
    }

    #[test]
    fn empty_hot_cue_set_while_paused_stays_paused() {
        let mut deck = loaded_deck();
        deck.seek(31_000.0);
        deck.hot_cue_press(0);
        deck.hot_cue_release(0);
        let s = deck.state();
        assert_eq!(s.hot_cues[0], Some(31_000.0));
        assert!(!s.playing);
        assert_eq!(s.position, 31_000.0);
    }

    #[test]
    fn hot_cue_without_a_track_does_nothing() {
        let mut deck = Deck::new();
        deck.hot_cue_press(0);
        assert_eq!(deck.state().hot_cues, [None; 8]);
    }

    #[test]
    fn hot_cue_jumps_and_keeps_playing() {
        let mut deck = loaded_deck();
        deck.seek(45_000.0);
        deck.hot_cue_press(1);
        deck.hot_cue_release(1);
        deck.seek(130_000.0);
        deck.play();
        deck.hot_cue_press(1);
        let s = deck.state();
        assert_eq!(s.position, 45_000.0);
        assert!(s.playing);
        deck.hot_cue_release(1);
        assert!(deck.is_playing(), "release does nothing while playing");
    }

    #[test]
    fn hot_cue_previews_while_held_when_paused() {
        let mut deck = loaded_deck();
        deck.seek(20_000.0);
        deck.hot_cue_press(3);
        deck.hot_cue_release(3);
        deck.seek(90_000.0);
        deck.hot_cue_press(3);
        assert!(deck.is_playing());
        render_frames(&mut deck, 1_000, 2_000);
        assert!(deck.state().position > 20_000.0);
        deck.hot_cue_release(3);
        let s = deck.state();
        assert!(!s.playing);
        assert_eq!(s.position, 20_000.0);
    }

    #[test]
    fn play_during_hot_cue_preview_keeps_playing() {
        let mut deck = loaded_deck();
        deck.hot_cue_press(4); // set at 0
        deck.hot_cue_release(4);
        deck.seek(5_000.0);
        deck.hot_cue_press(4);
        deck.play();
        deck.hot_cue_release(4);
        assert!(deck.is_playing());
    }

    #[test]
    fn cleared_hot_cue_is_empty_and_playback_unchanged() {
        let mut deck = loaded_deck();
        deck.seek(10_000.0);
        deck.hot_cue_press(2);
        deck.play();
        deck.clear_hot_cue(2);
        let s = deck.state();
        assert_eq!(s.hot_cues[2], None);
        assert!(s.playing);
        assert_eq!(s.position, 10_000.0);
    }

    #[test]
    fn clear_all_hot_cues_keeps_playback() {
        let mut deck = loaded_deck();
        for (slot, at) in [(0, 1_000.0), (3, 2_000.0), (7, 10_000.0)] {
            deck.seek(at);
            deck.hot_cue_press(slot);
            deck.hot_cue_release(slot);
        }
        deck.play();
        deck.clear_hot_cues(2); // another track: ignored
        assert!(deck.state().hot_cues[0].is_some());
        deck.clear_hot_cues(1);
        let s = deck.state();
        assert!(s.hot_cues.iter().all(Option::is_none));
        assert!(s.playing);
        assert_eq!(s.position, 10_000.0);
    }

    #[test]
    fn load_takes_the_tracks_hot_cues() {
        let mut deck = loaded_deck();
        deck.hot_cue_press(0);
        let mut cues = [None; 8];
        cues[1] = Some(500.0);
        cues[7] = Some(1e12); // past the end: dropped
        deck.load(sine(440.0, 1_000, 10.0), 2, cues);
        let s = deck.state();
        assert_eq!(s.hot_cues[0], None);
        assert_eq!(s.hot_cues[1], Some(500.0));
        assert_eq!(s.hot_cues[7], None);
    }

    #[test]
    fn hot_cue_set_at_the_end_survives_a_reload() {
        let mut deck = Deck::new();
        deck.load(sine(440.0, 1_000, 1.0), 1, Default::default());
        deck.play();
        render_frames(&mut deck, 1_000, 2_000); // plays to the end and stops
        deck.hot_cue_press(0);
        let cues = deck.state().hot_cues;
        assert_eq!(cues[0], Some(1_000.0));
        deck.load(sine(440.0, 1_000, 1.0), 1, cues);
        assert_eq!(deck.state().hot_cues[0], Some(1_000.0));
    }

    // ---- Loops

    /// 128 BPM at the 1 kHz test rate: one beat is 468.75 frames.
    const BEAT_128: f64 = 60.0 / 128.0 * 1_000.0;

    fn looping_deck() -> Deck {
        let mut deck = loaded_deck();
        deck.set_beat_length(1, Some(BEAT_128));
        deck
    }

    #[test]
    fn loop_length_comes_from_the_grid_not_the_pitch() {
        let mut deck = looping_deck();
        deck.set_pitch(0.04);
        deck.seek(10_000.0);
        deck.play();
        deck.beat_loop(4.0);
        let l = deck.state().active_loop.unwrap();
        assert_eq!(l.end - l.start, 1_875.0, "4 beats of 128 BPM = 1.875 s");
    }

    #[test]
    fn loop_starts_exactly_at_the_playhead() {
        let mut deck = looping_deck();
        deck.seek(60_240.0);
        deck.play();
        deck.beat_loop(2.0);
        let l = deck.state().active_loop.unwrap();
        assert_eq!(l.start, 60_240.0, "not snapped to any beat");
        assert_eq!(l.end, 60_240.0 + 2.0 * BEAT_128);
        render_frames(&mut deck, 1_000, 5_000);
        let p = deck.state().position;
        assert!((l.start..l.end).contains(&p), "still looping at {p}");
    }

    #[test]
    fn loop_set_while_paused_loops_after_play() {
        let mut deck = looping_deck();
        deck.seek(5_000.0);
        deck.beat_loop(8.0);
        assert!(!deck.is_playing());
        deck.play();
        render_frames(&mut deck, 1_000, 20_000);
        let l = deck.state().active_loop.unwrap();
        assert!((l.start..l.end).contains(&deck.state().position));
    }

    #[test]
    fn pressing_the_active_length_exits_the_loop() {
        let mut deck = looping_deck();
        deck.play();
        deck.beat_loop(4.0);
        deck.beat_loop(4.0);
        assert_eq!(deck.state().active_loop, None);
        render_frames(&mut deck, 1_000, 5_000);
        assert!(
            deck.state().position > 4.0 * BEAT_128,
            "plays on past the end"
        );
    }

    #[test]
    fn another_length_resizes_from_the_same_start() {
        let mut deck = looping_deck();
        deck.seek(40_000.0);
        deck.play();
        deck.beat_loop(8.0);
        render_frames(&mut deck, 1_000, 3_000); // into the second half
        deck.beat_loop(4.0);
        let l = deck.state().active_loop.unwrap();
        assert_eq!(l.start, 40_000.0);
        assert_eq!(l.end, 40_000.0 + 4.0 * BEAT_128);
        assert_eq!(l.beats, 4.0);
        let p = deck.state().position;
        assert!(
            (l.start..l.end).contains(&p),
            "wrapped into the new loop: {p}"
        );
    }

    #[test]
    fn leaving_the_loop_region_ends_the_loop() {
        let mut deck = looping_deck();
        deck.seek(150_000.0);
        deck.hot_cue_press(0); // hot cue at 2:30
        deck.seek(60_000.0);
        deck.play();
        deck.beat_loop(4.0);
        deck.hot_cue_press(0);
        assert_eq!(deck.state().active_loop, None);
        assert_eq!(deck.state().position, 150_000.0);

        // A seek inside the loop keeps it; one outside ends it.
        deck.beat_loop(4.0);
        deck.seek(150_100.0);
        assert!(deck.state().active_loop.is_some());
        deck.seek(10.0);
        assert_eq!(deck.state().active_loop, None);
    }

    #[test]
    fn loop_without_a_tempo_is_refused_and_counted() {
        let mut deck = loaded_deck();
        deck.play();
        deck.beat_loop(4.0);
        let s = deck.state();
        assert_eq!(s.active_loop, None);
        assert_eq!(s.loop_refusals, 1);
    }

    #[test]
    fn beat_length_for_another_track_is_ignored() {
        let mut deck = loaded_deck();
        deck.set_beat_length(99, Some(500.0));
        assert_eq!(deck.state().beat_frames, None);
    }

    #[test]
    fn loop_near_the_end_stops_at_the_end() {
        let mut deck = looping_deck();
        deck.seek(195_000.0); // 5 s before the end
        deck.play();
        deck.beat_loop(32.0);
        let l = deck.state().active_loop.unwrap();
        assert_eq!(l.end, 200_000.0);
        render_frames(&mut deck, 1_000, 12_000);
        let s = deck.state();
        assert!(s.playing, "repeats rather than stopping at the end");
        assert!((l.start..l.end).contains(&s.position));
    }

    #[test]
    fn loop_wrap_is_seamless_exact_and_never_overshoots() {
        let rate = 48_000;
        let mut deck = Deck::new();
        deck.load(sine(440.0, rate, 10.0), 1, Default::default());
        // 1/4 beat at 174 BPM: the shortest common loop (~86 ms).
        let beat = 60.0 / 174.0 * f64::from(rate);
        deck.set_beat_length(1, Some(beat));
        deck.seek(12_345.0);
        deck.play();
        deck.beat_loop(0.25);
        let l = deck.state().active_loop.unwrap();
        let len = l.end - l.start;

        let block = 256;
        let mut lb = vec![0.0; block];
        let mut rb = vec![0.0; block];
        let mut prev: Option<f32> = None;
        let mut max_step = 0.0f32;
        let mut frames = 0usize;
        while (frames as f64) < 1_000.0 * len {
            deck.render(rate, &mut lb, &mut rb);
            frames += block;
            let p = deck.state().position;
            assert!(p >= l.start && p < l.end, "position {p} outside the loop");
            for &v in &lb {
                if let Some(pv) = prev {
                    max_step = max_step.max((v - pv).abs());
                }
                prev = Some(v);
            }
        }
        // A 440 Hz sine at 0.5 moves at most ~0.029 per sample; an unfaded jump would
        // be up to 1.0. The crossfade keeps every step close to the sine's own slope.
        assert!(max_step < 0.06, "largest step {max_step}");
        let expected = l.start + (frames as f64) % len;
        let p = deck.state().position;
        assert!((p - expected).abs() < 1e-3, "drifted: {p} vs {expected}");
    }
}
