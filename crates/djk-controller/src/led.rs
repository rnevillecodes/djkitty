//! LED feedback: desired LED state from the engine snapshot, diffed against what was last
//! sent so only changes go over MIDI.

use djk_core::{DECK_COUNT, DeckId, EngineState};

use crate::mapping::Mapping;

/// Blink rate for the cue LED when paused away from the cue point.
pub const BLINK_HZ: f64 = 2.0;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeckLeds {
    pub play: bool,
    pub cue: bool,
    pub headphone_cue: bool,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LedState {
    pub decks: [DeckLeds; DECK_COUNT],
}

/// What the LEDs should show. `time_secs` drives blinking.
pub fn desired(state: &EngineState, time_secs: f64) -> LedState {
    let blink_on = (time_secs * BLINK_HZ * 2.0).floor() as i64 % 2 == 0;
    let mut out = LedState::default();
    for deck in DeckId::ALL {
        let d = state.deck(deck);
        let cue = if !d.loaded {
            false
        } else if d.playing {
            d.previewing
        } else if d.at_cue() {
            true
        } else {
            blink_on
        };
        out.decks[deck.index()] = DeckLeds {
            play: d.loaded && d.playing && !d.previewing,
            cue,
            headphone_cue: state.mixer.headphone_cue[deck.index()],
        };
    }
    out
}

/// MIDI messages that turn `from` into `to`. Pass `from = None` for a full resync.
pub fn messages(mapping: &Mapping, from: Option<&LedState>, to: &LedState) -> Vec<[u8; 3]> {
    let mut out = Vec::new();
    for deck in DeckId::ALL {
        let status = mapping.deck_note_status(deck);
        let new = to.decks[deck.index()];
        let old = from.map(|f| f.decks[deck.index()]);
        for (note, on, was) in [
            (mapping.leds.play, new.play, old.map(|o| o.play)),
            (mapping.leds.cue, new.cue, old.map(|o| o.cue)),
            (
                mapping.leds.headphone_cue,
                new.headphone_cue,
                old.map(|o| o.headphone_cue),
            ),
        ] {
            if was != Some(on) {
                out.push([status, note, if on { 0x7F } else { 0x00 }]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::DDJ_FLX2;

    fn state() -> EngineState {
        let mut s = EngineState::default();
        for d in &mut s.decks {
            d.loaded = true;
            d.track_frames = 44_100 * 60;
        }
        s
    }

    #[test]
    fn play_led_lights_when_deck_b_plays() {
        let mut s = state();
        let before = desired(&s, 0.0);
        s.decks[1].playing = true;
        let after = desired(&s, 0.0);
        assert!(after.decks[1].play);
        let msgs = messages(&DDJ_FLX2, Some(&before), &after);
        assert!(msgs.contains(&[0x91, 0x0B, 0x7F]), "{msgs:02X?}");
        assert!(!msgs.iter().any(|m| m[0] == 0x90), "deck A unchanged");
    }

    #[test]
    fn cue_led_on_at_cue_blinks_elsewhere() {
        let mut s = state();
        assert!(desired(&s, 0.0).decks[0].cue, "paused at cue");
        s.decks[0].position = 44_100.0 * 10.0;
        assert!(desired(&s, 0.0).decks[0].cue);
        assert!(!desired(&s, 0.3).decks[0].cue, "blinks");
        s.decks[0].playing = true;
        assert!(!desired(&s, 0.0).decks[0].cue, "off while playing");
    }

    #[test]
    fn headphone_cue_led_follows_mixer() {
        let mut s = state();
        s.mixer.headphone_cue[0] = true;
        assert!(desired(&s, 0.0).decks[0].headphone_cue);
        assert!(!desired(&s, 0.0).decks[1].headphone_cue);
    }

    #[test]
    fn resync_sends_every_led_and_diff_sends_nothing_when_unchanged() {
        let mut s = state();
        s.decks[0].playing = true;
        let leds = desired(&s, 0.0);
        let full = messages(&DDJ_FLX2, None, &leds);
        assert_eq!(full.len(), 6);
        assert!(
            full.contains(&[0x90, 0x0B, 0x7F]),
            "play A lit after reconnect"
        );
        assert!(messages(&DDJ_FLX2, Some(&leds), &leds).is_empty());
    }
}
