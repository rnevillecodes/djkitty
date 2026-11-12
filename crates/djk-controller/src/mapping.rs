//! Controller mappings as data: MIDI messages → controls → actions.

use djk_core::{Command, DeckId, LOOP_BEATS, PITCH_RANGE, PadMode};

use crate::takeover::{FaderTarget, Takeover};

/// A button with an action.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Button {
    Play(DeckId),
    Cue(DeckId),
    HeadphoneCue(DeckId),
    Load(DeckId),
}

/// Relative encoders, centred on 64.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Encoder {
    Jog(DeckId),
    Browse,
}

/// Which half of a 14-bit control a CC carries.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Msb,
    Lsb,
}

/// What a MIDI message (status, data1) means.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Control {
    Button(Button),
    Fader(FaderTarget, Part),
    Encoder(Encoder),
    /// A deck's SHIFT button.
    Shift(DeckId),
    /// BEAT SYNC: never syncs. With SHIFT (held, or sent as its own `shifted` message)
    /// it enters or leaves pad mode selection.
    BeatSync {
        deck: DeckId,
        shifted: bool,
    },
    /// A performance pad (`slot` 0–7). `mode` is the mode the message itself belongs to,
    /// when the hardware sends different notes per mode, or `None` to use the deck's
    /// current mode. `shifted` when the message is SHIFT's variant of the pad.
    Pad {
        deck: DeckId,
        mode: Option<PadMode>,
        slot: u8,
        shifted: bool,
    },
}

/// Where a controller's performance pads sit in MIDI: a note channel per deck and a
/// block of 8 notes per mode, with SHIFT's variants `shift_offset` notes higher. The
/// same notes light the pads.
#[derive(Copy, Clone, Debug)]
pub struct PadLayout {
    /// Note-on status byte for each deck's pads.
    pub status: [u8; 2],
    pub hot_cue: u8,
    pub beat_loop: u8,
    pub shift_offset: u8,
}

impl PadLayout {
    /// First note of a mode's block.
    pub fn base(&self, mode: PadMode) -> u8 {
        match mode {
            PadMode::HotCue => self.hot_cue,
            PadMode::BeatLoop => self.beat_loop,
        }
    }

    fn lookup(&self, status: u8, note: u8) -> Option<Control> {
        let deck = DeckId::ALL
            .into_iter()
            .find(|d| self.status[d.index()] == status)?;
        [PadMode::HotCue, PadMode::BeatLoop]
            .into_iter()
            .find_map(|mode| {
                let base = self.base(mode);
                [(base, false), (base + self.shift_offset, true)]
                    .into_iter()
                    .find(|(b, _)| (*b..*b + 8).contains(&note))
                    .map(|(b, shifted)| Control::Pad {
                        deck,
                        mode: Some(mode),
                        slot: note - b,
                        shifted,
                    })
            })
    }
}

/// Something the controller asks the app to do.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    /// Sent straight to the engine.
    Engine(Command),
    /// Move the track-list selection by this many rows (positive = down).
    Browse(i32),
    /// Load the selected track onto a deck.
    LoadSelected(DeckId),
}

/// A static controller mapping.
pub struct Mapping {
    pub name: &'static str,
    /// Substring of the MIDI port name that identifies the controller.
    pub port_match: &'static str,
    pub entries: &'static [((u8, u8), Control)],
    /// Pitch fader direction: true if a higher 14-bit value means faster.
    pub pitch_up_is_faster: bool,
    /// LED note for each button LED, sent on the deck's note channel.
    pub leds: LedMap,
    /// Sent on connect to ask the controller to report every fader and knob position,
    /// so the app matches the hardware before anything is moved.
    pub request_state: Option<&'static [u8]>,
    pub pads: Option<PadLayout>,
}

#[derive(Copy, Clone, Debug)]
pub struct LedMap {
    pub play: u8,
    pub cue: u8,
    pub headphone_cue: u8,
}

impl Mapping {
    pub fn lookup(&self, status: u8, data1: u8) -> Option<Control> {
        // Treat note-off as note-on with velocity 0.
        let status = if status & 0xF0 == 0x80 {
            0x90 | (status & 0x0F)
        } else {
            status
        };
        self.entries
            .iter()
            .find(|((s, d), _)| *s == status && *d == data1)
            .map(|(_, c)| *c)
            .or_else(|| self.pads.and_then(|p| p.lookup(status, data1)))
    }

    /// Note-on status byte for a deck's buttons and LEDs.
    pub fn deck_note_status(&self, deck: DeckId) -> u8 {
        0x90 | deck.index() as u8
    }
}

use Button as B;
use Control as C;
use DeckId::{A, B as DB};
use Encoder as E;
use FaderTarget as F;
use Part::{Lsb, Msb};
use djk_core::EqBand::{High, Low, Mid};

/// Pioneer DJ / AlphaTheta DDJ-FLX2.
///
/// PROVISIONAL: message numbers follow the DDJ-FLX4 conventions and must be confirmed on
/// hardware with `--midi-monitor` (see `docs/controllers/ddj-flx2.md`).
pub static DDJ_FLX2: Mapping = Mapping {
    name: "DDJ-FLX2",
    port_match: "DDJ-FLX2",
    pitch_up_is_faster: false,
    leds: LedMap {
        play: 0x0B,
        cue: 0x0C,
        headphone_cue: 0x54,
    },
    // The DDJ-400/FLX4 "send current control positions" SysEx (as used by Mixxx).
    request_state: Some(&[
        0xF0, 0x00, 0x40, 0x05, 0x00, 0x00, 0x02, 0x06, 0x00, 0x03, 0x01, 0xF7,
    ]),
    // FLX4 layout: deck 1 pads on channel 8, deck 2 on channel 10; Hot Cue notes 0x00–0x07,
    // Beat Loop 0x60–0x67, SHIFT variants 8 notes higher.
    pads: Some(PadLayout {
        status: [0x97, 0x99],
        hot_cue: 0x00,
        beat_loop: 0x60,
        shift_offset: 0x08,
    }),
    entries: &[
        // Deck 1 (MIDI channel 1)
        ((0x90, 0x0B), C::Button(B::Play(A))),
        ((0x90, 0x0C), C::Button(B::Cue(A))),
        ((0x90, 0x54), C::Button(B::HeadphoneCue(A))),
        ((0x90, 0x3F), C::Shift(A)),
        (
            (0x90, 0x58),
            C::BeatSync {
                deck: A,
                shifted: false,
            },
        ),
        (
            (0x90, 0x5C),
            C::BeatSync {
                deck: A,
                shifted: true,
            },
        ),
        ((0xB0, 0x00), C::Fader(F::Pitch(A), Msb)),
        ((0xB0, 0x20), C::Fader(F::Pitch(A), Lsb)),
        ((0xB0, 0x13), C::Fader(F::Channel(A), Msb)),
        ((0xB0, 0x33), C::Fader(F::Channel(A), Lsb)),
        ((0xB0, 0x07), C::Fader(F::Eq(A, High), Msb)),
        ((0xB0, 0x27), C::Fader(F::Eq(A, High), Lsb)),
        ((0xB0, 0x0B), C::Fader(F::Eq(A, Mid), Msb)),
        ((0xB0, 0x2B), C::Fader(F::Eq(A, Mid), Lsb)),
        ((0xB0, 0x0F), C::Fader(F::Eq(A, Low), Msb)),
        ((0xB0, 0x2F), C::Fader(F::Eq(A, Low), Lsb)),
        ((0xB0, 0x21), C::Encoder(E::Jog(A))),
        ((0xB0, 0x22), C::Encoder(E::Jog(A))),
        ((0xB0, 0x23), C::Encoder(E::Jog(A))),
        // Deck 2 (MIDI channel 2)
        ((0x91, 0x0B), C::Button(B::Play(DB))),
        ((0x91, 0x0C), C::Button(B::Cue(DB))),
        ((0x91, 0x54), C::Button(B::HeadphoneCue(DB))),
        ((0x91, 0x3F), C::Shift(DB)),
        (
            (0x91, 0x58),
            C::BeatSync {
                deck: DB,
                shifted: false,
            },
        ),
        (
            (0x91, 0x5C),
            C::BeatSync {
                deck: DB,
                shifted: true,
            },
        ),
        ((0xB1, 0x00), C::Fader(F::Pitch(DB), Msb)),
        ((0xB1, 0x20), C::Fader(F::Pitch(DB), Lsb)),
        ((0xB1, 0x13), C::Fader(F::Channel(DB), Msb)),
        ((0xB1, 0x33), C::Fader(F::Channel(DB), Lsb)),
        ((0xB1, 0x07), C::Fader(F::Eq(DB, High), Msb)),
        ((0xB1, 0x27), C::Fader(F::Eq(DB, High), Lsb)),
        ((0xB1, 0x0B), C::Fader(F::Eq(DB, Mid), Msb)),
        ((0xB1, 0x2B), C::Fader(F::Eq(DB, Mid), Lsb)),
        ((0xB1, 0x0F), C::Fader(F::Eq(DB, Low), Msb)),
        ((0xB1, 0x2F), C::Fader(F::Eq(DB, Low), Lsb)),
        ((0xB1, 0x21), C::Encoder(E::Jog(DB))),
        ((0xB1, 0x22), C::Encoder(E::Jog(DB))),
        ((0xB1, 0x23), C::Encoder(E::Jog(DB))),
        // Mixer / browser (MIDI channel 7)
        ((0xB6, 0x1F), C::Fader(F::Crossfader, Msb)),
        ((0xB6, 0x3F), C::Fader(F::Crossfader, Lsb)),
        ((0xB6, 0x40), C::Encoder(E::Browse)),
        ((0x96, 0x46), C::Button(B::Load(A))),
        ((0x96, 0x47), C::Button(B::Load(DB))),
    ],
};

/// Per-connection translation state: pending 14-bit MSBs and soft takeover.
pub struct Translator {
    pub mapping: &'static Mapping,
    msb: [u8; FaderTarget::COUNT],
    pub takeover: Takeover,
    /// Whether each deck's SHIFT is held.
    shift: [bool; 2],
    /// The pad each deck pressed to choose its mode, whose release is ignored.
    choosing_pad: [Option<u8>; 2],
}

impl Translator {
    pub fn new(mapping: &'static Mapping, takeover: Takeover) -> Self {
        Self {
            mapping,
            msb: [0; FaderTarget::COUNT],
            takeover,
            shift: [false; 2],
            choosing_pad: [None; 2],
        }
    }

    /// Translate one raw MIDI message. Unmapped messages yield `None`.
    pub fn translate(&mut self, msg: &[u8]) -> Option<Action> {
        let (&status, rest) = msg.split_first()?;
        let data1 = *rest.first()?;
        let value = rest.get(1).copied().unwrap_or(0);
        let note_off = status & 0xF0 == 0x80;
        match self.mapping.lookup(status, data1)? {
            C::Button(b) => {
                let pressed = !note_off && value > 0;
                button_action(b, pressed)
            }
            C::Fader(target, part) => {
                let raw = match part {
                    Msb => {
                        self.msb[target.index()] = value;
                        u16::from(value) << 7
                    }
                    Lsb => (u16::from(self.msb[target.index()]) << 7) | u16::from(value),
                };
                let normalised = f32::from(raw) / 16383.0;
                let hw = match target {
                    F::Pitch(_) => {
                        let up = if self.mapping.pitch_up_is_faster {
                            normalised
                        } else {
                            1.0 - normalised
                        };
                        (up * 2.0 - 1.0) * PITCH_RANGE
                    }
                    _ => normalised,
                };
                let applied = self.takeover.apply(target, hw)?;
                Some(Action::Engine(fader_command(target, applied)))
            }
            C::Encoder(e) => {
                let delta = i32::from(value) - 64;
                if delta == 0 {
                    return None;
                }
                match e {
                    E::Jog(deck) => Some(Action::Engine(Command::Jog {
                        deck,
                        ticks: delta as f32,
                    })),
                    E::Browse => Some(Action::Browse(delta)),
                }
            }
            C::Shift(deck) => {
                self.shift[deck.index()] = !note_off && value > 0;
                None
            }
            C::BeatSync { deck, shifted } => {
                // BEAT SYNC never syncs: its only job is SHIFT + BEAT SYNC → pad modes.
                let pressed = !note_off && value > 0;
                if pressed && (shifted || self.shift[deck.index()]) {
                    let values = self.takeover.values();
                    values.set_selecting(deck, !values.selecting(deck));
                }
                None
            }
            C::Pad {
                deck,
                mode,
                slot,
                shifted,
            } => {
                let pressed = !note_off && value > 0;
                let shifted = shifted || self.shift[deck.index()];
                // The pad pressed to choose a mode does nothing when released.
                if !pressed && self.choosing_pad[deck.index()] == Some(slot) {
                    self.choosing_pad[deck.index()] = None;
                    return None;
                }
                if pressed && self.takeover.values().selecting(deck) {
                    self.choosing_pad[deck.index()] = Some(slot);
                }
                pad_action(self.takeover.values(), deck, mode, slot, shifted, pressed)
            }
        }
    }
}

/// What a pad does. During mode selection, pad 1 picks Hot Cue and pad 3 Beat Loop (any
/// pad ends selection; Pad FX and Sampler are not supported, so they keep the mode).
/// Otherwise the pad acts in the mode its message belongs to (which the deck then takes
/// on, when the hardware tracks the mode itself) or the deck's current mode.
fn pad_action(
    values: &crate::takeover::AppValues,
    deck: DeckId,
    mode: Option<PadMode>,
    slot: u8,
    shifted: bool,
    pressed: bool,
) -> Option<Action> {
    if values.selecting(deck) {
        if pressed {
            values.set_selecting(deck, false);
            match slot {
                0 => values.set_pad_mode(deck, PadMode::HotCue),
                2 => values.set_pad_mode(deck, PadMode::BeatLoop),
                _ => {}
            }
        }
        return None;
    }
    let mode = match mode {
        Some(m) if pressed => {
            values.set_pad_mode(deck, m);
            m
        }
        _ => values.pad_mode(deck),
    };
    let cmd = match (mode, pressed, shifted) {
        (PadMode::HotCue, true, true) => Command::ClearHotCue { deck, slot },
        (PadMode::HotCue, true, false) => Command::HotCuePress { deck, slot },
        (PadMode::HotCue, false, _) => Command::HotCueRelease { deck, slot },
        (PadMode::BeatLoop, true, false) => Command::BeatLoop {
            deck,
            beats: *LOOP_BEATS.get(usize::from(slot))?,
        },
        (PadMode::BeatLoop, _, _) => return None,
    };
    Some(Action::Engine(cmd))
}

fn button_action(b: Button, pressed: bool) -> Option<Action> {
    let cmd = match (b, pressed) {
        (B::Play(d), true) => Command::TogglePlay(d),
        (B::Cue(d), true) => Command::CuePress(d),
        (B::Cue(d), false) => Command::CueRelease(d),
        (B::HeadphoneCue(d), true) => Command::ToggleHeadphoneCue(d),
        (B::Load(d), true) => return Some(Action::LoadSelected(d)),
        _ => return None,
    };
    Some(Action::Engine(cmd))
}

fn fader_command(target: FaderTarget, value: f32) -> Command {
    match target {
        F::Channel(deck) => Command::SetChannelFader { deck, value },
        F::Crossfader => Command::SetCrossfader(value),
        F::Pitch(deck) => Command::SetPitch { deck, pitch: value },
        F::Eq(deck, band) => Command::SetEq { deck, band, value },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::takeover::AppValues;
    use std::sync::Arc;

    fn translator() -> (Translator, Arc<AppValues>) {
        let values = Arc::new(AppValues::default());
        (
            Translator::new(&DDJ_FLX2, Takeover::new(values.clone())),
            values,
        )
    }

    fn engine(a: Option<Action>) -> Command {
        match a {
            Some(Action::Engine(c)) => c,
            other => panic!("expected engine command, got {other:?}"),
        }
    }

    #[test]
    fn play_and_cue_buttons() {
        let (mut t, _) = translator();
        assert_eq!(
            engine(t.translate(&[0x90, 0x0B, 0x7F])),
            Command::TogglePlay(A)
        );
        assert_eq!(t.translate(&[0x90, 0x0B, 0x00]), None, "release ignored");
        assert_eq!(
            engine(t.translate(&[0x91, 0x0C, 0x7F])),
            Command::CuePress(DB)
        );
        assert_eq!(
            engine(t.translate(&[0x81, 0x0C, 0x00])),
            Command::CueRelease(DB),
            "note-off releases"
        );
        assert_eq!(
            engine(t.translate(&[0x90, 0x54, 0x7F])),
            Command::ToggleHeadphoneCue(A)
        );
    }

    #[test]
    fn crossfader_fully_right_after_takeover() {
        let (mut t, _) = translator();
        // App crossfader starts centred (0.5): sweep the hardware from the left past it.
        for v in [0u8, 30, 63, 64, 100, 127] {
            t.translate(&[0xB6, 0x1F, v]);
        }
        let cmd = engine(t.translate(&[0xB6, 0x3F, 0x7F]));
        assert_eq!(cmd, Command::SetCrossfader(1.0));
    }

    #[test]
    fn fourteen_bit_fader_combines_msb_and_lsb() {
        let (mut t, values) = translator();
        values.set(FaderTarget::Channel(A), 0.5);
        t.translate(&[0xB0, 0x13, 0x40]); // MSB 64 → 0.5004, engages near app value
        let cmd = engine(t.translate(&[0xB0, 0x33, 0x20]));
        let Command::SetChannelFader { deck, value } = cmd else {
            panic!("{cmd:?}")
        };
        assert_eq!(deck, A);
        let expected = ((64u16 << 7) | 32) as f32 / 16383.0;
        assert!((value - expected).abs() < 1e-6);
    }

    #[test]
    fn eq_knob_kills_after_picking_up_the_centre() {
        let (mut t, values) = translator();
        // EQ centred on screen: a hardware knob far from it is ignored...
        values.set_from_ui(FaderTarget::Eq(DB, Low), djk_core::EQ_NEUTRAL);
        assert_eq!(t.translate(&[0xB1, 0x0F, 0x00]), None);
        // ...until it passes the centre, then it follows the knob down to a kill.
        t.translate(&[0xB1, 0x0F, 0x40]);
        t.translate(&[0xB1, 0x0F, 0x00]);
        let cmd = engine(t.translate(&[0xB1, 0x2F, 0x00]));
        assert_eq!(
            cmd,
            Command::SetEq {
                deck: DB,
                band: Low,
                value: 0.0
            }
        );
        // The deck A HI knob maps too.
        t.translate(&[0xB0, 0x07, 0x40]);
        let Command::SetEq { deck, band, .. } = engine(t.translate(&[0xB0, 0x27, 0x00])) else {
            panic!()
        };
        assert_eq!((deck, band), (A, High));
    }

    #[test]
    fn pitch_fader_maps_to_range() {
        let (mut t, values) = translator();
        values.set(FaderTarget::Pitch(DB), 0.0);
        // Centre engages, then the bottom of the fader (max value) is slowest on FLX4.
        t.translate(&[0xB1, 0x00, 0x40]);
        t.translate(&[0xB1, 0x20, 0x00]);
        t.translate(&[0xB1, 0x00, 0x7F]);
        let Command::SetPitch { deck, pitch } = engine(t.translate(&[0xB1, 0x20, 0x7F])) else {
            panic!()
        };
        assert_eq!(deck, DB);
        assert!((pitch + PITCH_RANGE).abs() < 1e-6, "{pitch}");
    }

    #[test]
    fn jog_and_browse_encoders() {
        let (mut t, _) = translator();
        assert_eq!(
            engine(t.translate(&[0xB0, 0x21, 0x43])),
            Command::Jog {
                deck: A,
                ticks: 3.0
            }
        );
        assert_eq!(
            engine(t.translate(&[0xB1, 0x22, 0x3E])),
            Command::Jog {
                deck: DB,
                ticks: -2.0
            }
        );
        assert_eq!(t.translate(&[0xB6, 0x40, 0x41]), Some(Action::Browse(1)));
        assert_eq!(t.translate(&[0xB6, 0x40, 0x3F]), Some(Action::Browse(-1)));
        assert_eq!(t.translate(&[0xB6, 0x40, 0x40]), None);
    }

    #[test]
    fn browse_twice_then_load() {
        let (mut t, _) = translator();
        let actions: Vec<_> = [[0xB6, 0x40, 0x41], [0xB6, 0x40, 0x41], [0x96, 0x46, 0x7F]]
            .iter()
            .filter_map(|m| t.translate(m))
            .collect();
        assert_eq!(
            actions,
            vec![
                Action::Browse(1),
                Action::Browse(1),
                Action::LoadSelected(A)
            ]
        );
    }

    #[test]
    fn unmapped_controls_are_ignored() {
        let (mut t, _) = translator();
        // Pads in unsupported modes (Pad FX), BEAT SYNC alone, FX, junk, short messages.
        for msg in [
            &[0x97, 0x10, 0x7F][..],
            &[0x90, 0x58, 0x7F],
            &[0x94, 0x47, 0x7F],
            &[0xB4, 0x02, 0x40],
            &[0xF0, 0x00],
            &[0x90],
            &[],
        ] {
            assert_eq!(t.translate(msg), None, "{msg:02X?}");
        }
    }

    // ---- Pads and pad modes (provisional FLX4 numbers: deck A pads on 0x97, B on 0x99)

    const SHIFT_A: u8 = 0x3F;
    const SYNC: u8 = 0x58;

    /// SHIFT + BEAT SYNC on a deck, then a pad (slot 0–7) to choose the mode.
    fn choose_mode(t: &mut Translator, deck_status: u8, pad_status: u8, slot: u8) {
        assert_eq!(t.translate(&[deck_status, SHIFT_A, 0x7F]), None);
        assert_eq!(t.translate(&[deck_status, SYNC, 0x7F]), None);
        assert_eq!(t.translate(&[deck_status, SYNC, 0x00]), None);
        assert_eq!(t.translate(&[deck_status, SHIFT_A, 0x00]), None);
        assert_eq!(
            t.translate(&[pad_status, slot, 0x7F]),
            None,
            "choosing does nothing else"
        );
        t.translate(&[pad_status, slot, 0x00]);
    }

    #[test]
    fn decks_start_in_hot_cue_mode() {
        let (_, values) = translator();
        assert_eq!(values.pad_mode(A), PadMode::HotCue);
        assert_eq!(values.pad_mode(DB), PadMode::HotCue);
    }

    #[test]
    fn shift_beat_sync_then_pad_3_switches_only_that_deck_to_beat_loop() {
        let (mut t, values) = translator();
        choose_mode(&mut t, 0x90, 0x97, 2);
        assert_eq!(values.pad_mode(A), PadMode::BeatLoop);
        assert_eq!(
            values.pad_mode(DB),
            PadMode::HotCue,
            "deck B keeps its mode"
        );
        assert!(!values.selecting(A), "selection ends");
        // And back with pad 1.
        choose_mode(&mut t, 0x90, 0x97, 0);
        assert_eq!(values.pad_mode(A), PadMode::HotCue);
    }

    #[test]
    fn shifted_beat_sync_message_also_enters_selection() {
        let (mut t, values) = translator();
        t.translate(&[0x91, 0x5C, 0x7F]);
        assert!(values.selecting(DB));
        assert!(!values.selecting(A));
    }

    #[test]
    fn unsupported_mode_choice_keeps_the_mode() {
        let (mut t, values) = translator();
        for slot in [1u8, 3] {
            choose_mode(&mut t, 0x91, 0x99, slot); // Pad FX, Sampler
            assert_eq!(values.pad_mode(DB), PadMode::HotCue);
        }
    }

    #[test]
    fn shift_beat_sync_again_leaves_selection_without_choosing() {
        let (mut t, values) = translator();
        t.translate(&[0x90, 0x5C, 0x7F]);
        assert!(values.selecting(A));
        t.translate(&[0x90, 0x5C, 0x7F]);
        assert!(!values.selecting(A));
        assert_eq!(values.pad_mode(A), PadMode::HotCue);
        // Pads act normally again.
        assert_eq!(
            engine(t.translate(&[0x97, 0x04, 0x7F])),
            Command::HotCuePress { deck: A, slot: 4 }
        );
    }

    #[test]
    fn beat_sync_alone_never_syncs() {
        let (mut t, values) = translator();
        assert_eq!(t.translate(&[0x91, SYNC, 0x7F]), None);
        assert_eq!(t.translate(&[0x91, SYNC, 0x00]), None);
        assert!(!values.selecting(DB));
    }

    #[test]
    fn hot_cue_pads_press_release_and_shift_clear() {
        let (mut t, _) = translator();
        assert_eq!(
            engine(t.translate(&[0x97, 0x04, 0x7F])),
            Command::HotCuePress { deck: A, slot: 4 }
        );
        assert_eq!(
            engine(t.translate(&[0x87, 0x04, 0x00])),
            Command::HotCueRelease { deck: A, slot: 4 },
            "note-off releases"
        );
        // SHIFT's own pad notes (8 higher) clear.
        assert_eq!(
            engine(t.translate(&[0x99, 0x0A, 0x7F])),
            Command::ClearHotCue { deck: DB, slot: 2 }
        );
        // So does a plain pad while that deck's SHIFT is held.
        t.translate(&[0x91, SHIFT_A, 0x7F]);
        assert_eq!(
            engine(t.translate(&[0x99, 0x02, 0x7F])),
            Command::ClearHotCue { deck: DB, slot: 2 }
        );
    }

    #[test]
    fn beat_loop_pads_map_to_loop_lengths() {
        let (mut t, values) = translator();
        choose_mode(&mut t, 0x91, 0x99, 2);
        // In Beat Loop mode the FLX4 sends its loop notes (0x60–0x67).
        assert_eq!(
            engine(t.translate(&[0x99, 0x64, 0x7F])),
            Command::BeatLoop {
                deck: DB,
                beats: 4.0
            },
            "pad 5 = 4 beats"
        );
        assert_eq!(
            engine(t.translate(&[0x99, 0x60, 0x7F])),
            Command::BeatLoop {
                deck: DB,
                beats: 0.25
            }
        );
        assert_eq!(
            engine(t.translate(&[0x99, 0x67, 0x7F])),
            Command::BeatLoop {
                deck: DB,
                beats: 32.0
            }
        );
        assert_eq!(
            t.translate(&[0x99, 0x64, 0x00]),
            None,
            "release does nothing"
        );
        assert_eq!(values.pad_mode(DB), PadMode::BeatLoop);
    }

    #[test]
    fn mode_agnostic_pads_follow_the_shared_mode() {
        // The screen switched deck A to Beat Loop; a pad whose message carries no mode
        // follows it.
        let (_, values) = translator();
        values.set_pad_mode(A, PadMode::BeatLoop);
        assert_eq!(
            pad_action(&values, A, None, 2, false, true),
            Some(Action::Engine(Command::BeatLoop {
                deck: A,
                beats: 1.0
            }))
        );
    }

    #[test]
    fn a_pad_in_another_mode_moves_the_deck_to_that_mode() {
        // Hardware that tracks the mode itself: its loop notes put the deck in Beat Loop.
        let (mut t, values) = translator();
        t.translate(&[0x97, 0x61, 0x7F]);
        assert_eq!(values.pad_mode(A), PadMode::BeatLoop);
    }
}
