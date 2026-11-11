//! Controller mappings as data: MIDI messages → controls → actions.

use djk_core::{Command, DeckId, PITCH_RANGE};

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
    entries: &[
        // Deck 1 (MIDI channel 1)
        ((0x90, 0x0B), C::Button(B::Play(A))),
        ((0x90, 0x0C), C::Button(B::Cue(A))),
        ((0x90, 0x54), C::Button(B::HeadphoneCue(A))),
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
}

impl Translator {
    pub fn new(mapping: &'static Mapping, takeover: Takeover) -> Self {
        Self {
            mapping,
            msb: [0; FaderTarget::COUNT],
            takeover,
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
        }
    }
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
        // Performance pads, sync, FX, shift layers, sysex-ish junk, short messages.
        for msg in [
            &[0x97, 0x00, 0x7F][..],
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
}
