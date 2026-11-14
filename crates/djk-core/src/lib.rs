//! Shared types for djkitty: identifiers, engine commands and state snapshots.
//!
//! Everything that crosses a thread boundary into or out of the real-time audio engine is
//! defined here, so the contract between the UI, the controller and the engine lives in one
//! place. This crate has no dependencies.

pub mod grid;
pub mod music;

use std::sync::Arc;
use std::time::Instant;

pub use grid::{BarBeat, Beatgrid};
pub use music::{Bpm, Key, Mode, PitchClass};

/// Number of decks in v0.1.
pub const DECK_COUNT: usize = 2;

/// Default pitch fader range (±8%).
pub const PITCH_RANGE: f32 = 0.08;

/// Hot cue slots per deck (one per performance pad).
pub const HOT_CUE_COUNT: usize = 8;

/// A deck's hot cues: a position in track frames per slot, or `None` when empty.
pub type HotCues = [Option<f64>; HOT_CUE_COUNT];

/// Beat loop lengths, in beats, for pads 1 to 8.
pub const LOOP_BEATS: [f64; HOT_CUE_COUNT] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];

/// What a deck's performance pads do.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum PadMode {
    #[default]
    HotCue,
    BeatLoop,
}

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
    /// Empty a deck (refused while it plays), e.g. when switching vaults. The track's
    /// buffer is retired off the audio thread, like the outgoing track of a load.
    Eject(DeckId),
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
    /// EQ knob position for one band of a deck, 0 (kill) to 1 (+6 dB), 0.5 = 0 dB.
    SetEq {
        deck: DeckId,
        band: EqBand,
        value: f32,
    },
    /// Filter knob position for a deck, 0 (full low-pass) to 1 (full high-pass), 0.5 = off.
    SetFilter { deck: DeckId, value: f32 },
    /// Hot cue pad pressed: set an empty slot at the playhead, jump to a set one while
    /// playing, or preview from it while paused (until released).
    HotCuePress { deck: DeckId, slot: u8 },
    /// Hot cue pad released: ends a preview started by that slot.
    HotCueRelease { deck: DeckId, slot: u8 },
    /// Empty a hot cue slot.
    ClearHotCue { deck: DeckId, slot: u8 },
    /// Empty all of a deck's hot cue slots at once (the on-screen "Clear hot cues"), if the
    /// deck holds the track in `token` (ignored for any other track, so a clear sent while
    /// a load is queued can't empty the wrong track's cues).
    ClearHotCues { deck: DeckId, token: u64 },
    /// Length of one beat of the track in `token`, in track frames (from its beatgrid),
    /// or 0 when it has no tempo (a plain `f64` keeps `Command` small). Ignored if the deck
    /// holds another track.
    SetBeatLength {
        deck: DeckId,
        token: u64,
        frames: f64,
    },
    /// Beat loop pad: start a loop this many beats long at the playhead, resize the
    /// active loop to it, or exit the loop if it already has this length.
    BeatLoop { deck: DeckId, beats: f64 },
    /// End the active loop and play on.
    ExitLoop(DeckId),
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
    /// The track's saved hot cues, in track frames. Carried with the load (rather than
    /// sent separately) so a refused load can't put them on the deck's current track.
    pub hot_cues: HotCues,
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
    pub hot_cues: HotCues,
    pub active_loop: Option<ActiveLoop>,
    /// One beat of the track in frames, if the engine knows its tempo.
    pub beat_frames: Option<f64>,
    /// Counts beat loops refused because the track has no tempo, so the UI can say why
    /// (including for presses that came straight from the controller).
    pub loop_refusals: u32,
    /// Counts loads refused because the deck was playing, so the UI can tell that a load
    /// it sent to this deck did not happen.
    pub load_refusals: u32,
}

/// A deck's active beat loop.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ActiveLoop {
    /// Start, in track frames.
    pub start: f64,
    /// End (exclusive), in track frames. Clamped to the end of the track.
    pub end: f64,
    /// The length it was started or resized to, in beats (one of [`LOOP_BEATS`]).
    pub beats: f64,
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
            hot_cues: [None; HOT_CUE_COUNT],
            active_loop: None,
            beat_frames: None,
            loop_refusals: 0,
            load_refusals: 0,
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

/// One band of a channel's 3-band EQ.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum EqBand {
    Low,
    Mid,
    High,
}

impl EqBand {
    /// Top to bottom, as on a mixer.
    pub const ALL: [EqBand; 3] = [EqBand::High, EqBand::Mid, EqBand::Low];

    /// Index into [`MixerState::eq`] (low, mid, high).
    pub const fn index(self) -> usize {
        match self {
            EqBand::Low => 0,
            EqBand::Mid => 1,
            EqBand::High => 2,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            EqBand::Low => "LOW",
            EqBand::Mid => "MID",
            EqBand::High => "HI",
        }
    }
}

/// EQ knob position for 0 dB (no change).
pub const EQ_NEUTRAL: f32 = 0.5;

/// Filter knob position that leaves the sound unchanged.
pub const FILTER_NEUTRAL: f32 = 0.5;

/// Snapshot of the mixer controls.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MixerState {
    pub channel_fader: [f32; DECK_COUNT],
    pub crossfader: f32,
    pub master_level: f32,
    pub headphone_level: f32,
    pub headphone_cue: [bool; DECK_COUNT],
    /// EQ knob positions per deck, `[low, mid, high]`, 0 (kill) to 1 (+6 dB), centre 0 dB.
    pub eq: [[f32; 3]; DECK_COUNT],
    /// Filter knob positions per deck, 0 (full low-pass) to 1 (full high-pass), centre off.
    pub filter: [f32; DECK_COUNT],
}

impl Default for MixerState {
    fn default() -> Self {
        Self {
            channel_fader: [1.0; DECK_COUNT],
            crossfader: 0.5,
            master_level: 1.0,
            headphone_level: 0.8,
            headphone_cue: [false; DECK_COUNT],
            eq: [[EQ_NEUTRAL; 3]; DECK_COUNT],
            filter: [FILTER_NEUTRAL; DECK_COUNT],
        }
    }
}

/// Everything the UI and controller LEDs need from the engine, published after each block.
#[derive(Copy, Clone, Debug)]
pub struct EngineState {
    pub decks: [DeckState; DECK_COUNT],
    pub mixer: MixerState,
    /// When the block that produced this snapshot was rendered, with the audio callback's
    /// scheduling jitter taken out: its end is heard `output_latency` after this.
    pub timestamp: Option<Instant>,
    /// Output latency in seconds (buffer plus device), measured by the stream and averaged.
    pub output_latency: f64,
    pub device_rate: u32,
    pub output_channels: u16,
    /// Audio blocks that missed their deadline.
    pub overruns: u64,
    /// Loads refused because the deck was playing.
    pub refused_loads: u64,
    pub frames_rendered: u64,
    /// Peak absolute sample per deck over the last block, before the channel fader
    /// (what a mixer's channel meter shows).
    pub channel_peak: [f32; DECK_COUNT],
    /// Peak absolute sample of the master output (left, right) over the last block,
    /// after the limiter.
    pub master_peak: [f32; 2],
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
            channel_peak: [0.0; DECK_COUNT],
            master_peak: [0.0; 2],
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
