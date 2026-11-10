//! Shared types for djkitty: identifiers, engine commands and state snapshots.
//!
//! Everything that crosses a thread boundary into or out of the real-time audio engine is
//! defined here, so the contract between the UI, the controller and the engine lives in one
//! place. This crate has no dependencies.

pub mod music;

use std::sync::Arc;
use std::time::Instant;

pub use music::{Bpm, Key, Mode, PitchClass};

/// Number of decks in v0.1.
pub const DECK_COUNT: usize = 2;

/// Default pitch fader range (±8%).
pub const PITCH_RANGE: f32 = 0.08;

/// One of the two decks.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DeckId {
    A,
    B,
}

impl DeckId {
    pub const ALL: [DeckId; DECK_COUNT] = [DeckId::A, DeckId::B];

    /// Zero-based index for array lookups.
    pub const fn index(self) -> usize {
        match self {
            DeckId::A => 0,
            DeckId::B => 1,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            DeckId::A => "A",
            DeckId::B => "B",
        }
    }
}

/// Library identifier of a track (the SQLite row id).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub i64);

/// A small, copyable instruction for the audio engine.
///
/// Track buffers are not `Copy`, so the buffer itself travels on a separate load queue as a
/// [`LoadRequest`] and [`Command::Load`] marks its place in the ordered command stream.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Command {
    /// Take the next [`LoadRequest`] from the load queue and put it on the deck (refused if
    /// the deck is playing). Sent by `EngineHandles::load`, never on its own.
    Load(DeckId),
    /// Start playback from the current position.
    Play(DeckId),
    /// Hold the current position.
    Pause(DeckId),
    /// Play if paused, pause if playing.
    TogglePlay(DeckId),
    /// CDJ-style cue button pressed.
    CuePress(DeckId),
    /// CDJ-style cue button released.
    CueRelease(DeckId),
    /// Move the playhead to a position in track frames.
    Seek { deck: DeckId, frame: f64 },
    /// Pitch as a fraction, e.g. `0.024` for +2.4%. Clamped to ±[`PITCH_RANGE`].
    SetPitch { deck: DeckId, pitch: f32 },
    /// Jog wheel movement in encoder ticks (positive = clockwise).
    Jog { deck: DeckId, ticks: f32 },
    /// Channel fader position, 0 (down) to 1 (up).
    SetChannelFader { deck: DeckId, value: f32 },
    /// Crossfader position, 0 (fully A) to 1 (fully B).
    SetCrossfader(f32),
    /// Master level, 0 to 1.
    SetMasterLevel(f32),
    /// Headphone level, 0 to 1.
    SetHeadphoneLevel(f32),
    /// Enable or disable headphone cue for a deck.
    SetHeadphoneCue { deck: DeckId, enabled: bool },
    /// Toggle headphone cue for a deck.
    ToggleHeadphoneCue(DeckId),
}

/// A fully decoded track: interleaved stereo `f32` at its native sample rate.
#[derive(Clone, Debug)]
pub struct TrackBuffer {
    pub samples: Arc<[f32]>,
    pub sample_rate: u32,
}

impl TrackBuffer {
    /// Number of stereo frames.
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames() as f64 / f64::from(self.sample_rate.max(1))
    }
}

/// Request to put a decoded track on a deck. Sent on the engine's load queue.
#[derive(Clone, Debug)]
pub struct LoadRequest {
    pub deck: DeckId,
    pub buffer: TrackBuffer,
    /// Opaque token echoed back in [`DeckState::track_token`] so the UI knows which
    /// track a deck holds (typically the [`TrackId`]).
    pub token: u64,
}

/// Snapshot of one deck, published by the engine after every audio block.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DeckState {
    pub loaded: bool,
    pub track_token: u64,
    pub track_frames: u64,
    pub track_rate: u32,
    /// Playhead in track frames (fractional).
    pub position: f64,
    pub playing: bool,
    /// True while cue is held for preview.
    pub previewing: bool,
    pub cue: f64,
    pub pitch: f32,
    /// Current speed multiplier relative to the track's own speed:
    /// `(1 + pitch) * (1 + jog nudge)`, or 0 when paused.
    pub speed: f64,
}

impl Default for DeckState {
    fn default() -> Self {
        Self {
            loaded: false,
            track_token: 0,
            track_frames: 0,
            track_rate: 44_100,
            position: 0.0,
            playing: false,
            previewing: false,
            cue: 0.0,
            pitch: 0.0,
            speed: 0.0,
        }
    }
}

impl DeckState {
    pub fn position_secs(&self) -> f64 {
        self.position / f64::from(self.track_rate.max(1))
    }

    pub fn duration_secs(&self) -> f64 {
        self.track_frames as f64 / f64::from(self.track_rate.max(1))
    }

    pub fn remaining_secs(&self) -> f64 {
        (self.duration_secs() - self.position_secs()).max(0.0)
    }

    /// True when the playhead sits on the cue point (within 1 ms).
    pub fn at_cue(&self) -> bool {
        (self.position - self.cue).abs() <= f64::from(self.track_rate) / 1000.0
    }
}

/// Snapshot of the mixer controls.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MixerState {
    pub channel_fader: [f32; DECK_COUNT],
    pub crossfader: f32,
    pub master_level: f32,
    pub headphone_level: f32,
    pub headphone_cue: [bool; DECK_COUNT],
}

impl Default for MixerState {
    fn default() -> Self {
        Self {
            channel_fader: [1.0; DECK_COUNT],
            crossfader: 0.5,
            master_level: 1.0,
            headphone_level: 0.8,
            headphone_cue: [false; DECK_COUNT],
        }
    }
}

/// Everything the UI and controller LEDs need from the engine, published after each block.
#[derive(Copy, Clone, Debug)]
pub struct EngineState {
    pub decks: [DeckState; DECK_COUNT],
    pub mixer: MixerState,
    /// When the block that produced this snapshot was rendered.
    pub timestamp: Option<Instant>,
    /// Output latency in seconds (buffer plus device), as measured by the stream.
    pub output_latency: f64,
    pub device_rate: u32,
    pub output_channels: u16,
    /// Audio blocks that missed their deadline.
    pub overruns: u64,
    /// Loads refused because the deck was playing.
    pub refused_loads: u64,
    pub frames_rendered: u64,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            decks: [DeckState::default(); DECK_COUNT],
            mixer: MixerState::default(),
            timestamp: None,
            output_latency: 0.0,
            device_rate: 48_000,
            output_channels: 2,
            overruns: 0,
            refused_loads: 0,
            frames_rendered: 0,
        }
    }
}

impl EngineState {
    pub fn deck(&self, deck: DeckId) -> &DeckState {
        &self.decks[deck.index()]
    }

    /// Whether headphone cue can be heard (needs a device with at least 4 channels).
    pub fn headphones_available(&self) -> bool {
        self.output_channels >= 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_copy<T: Copy>() {}

    #[test]
    fn command_is_copy_and_small() {
        assert_copy::<Command>();
        assert_copy::<EngineState>();
        assert!(std::mem::size_of::<Command>() <= 24);
    }

    #[test]
    fn deck_times() {
        let d = DeckState {
            loaded: true,
            track_frames: 44_100 * 360,
            track_rate: 44_100,
            position: 44_100.0 * 90.0,
            ..DeckState::default()
        };
        assert_eq!(d.position_secs(), 90.0);
        assert_eq!(d.remaining_secs(), 270.0);
    }
}
