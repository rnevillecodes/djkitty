//! LED feedback: desired LED state from the engine snapshot, diffed against what was last
//! sent so only changes go over MIDI.

use djk_core::{DECK_COUNT, DeckId, EngineState, HOT_CUE_COUNT, LOOP_BEATS, PadMode};

use crate::mapping::Mapping;
use crate::takeover::AppValues;

/// Blink rate for the cue LED when paused away from the cue point.
pub const BLINK_HZ: f64 = 2.0;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeckLeds {
    pub play: bool,
    pub cue: bool,
    pub headphone_cue: bool,
    /// The mode whose pad notes `pads` is sent on.
    pub pad_mode: PadMode,
    pub pads: [bool; HOT_CUE_COUNT],
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LedState {
    pub decks: [DeckLeds; DECK_COUNT],
}

/// What the LEDs should show. `time_secs` drives blinking; `pads` gives each deck's pad
/// mode and whether it is choosing one.
pub fn desired(state: &EngineState, pads: &AppValues, time_secs: f64) -> LedState {
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
        let pad_mode = pads.pad_mode(deck);
        let lit = if pads.selecting(deck) {
            // The modes that can be chosen: pad 1 (Hot Cue) and pad 3 (Beat Loop).
            std::array::from_fn(|i| i == 0 || i == 2)
        } else if !d.loaded {
            [false; HOT_CUE_COUNT]
        } else {
            match pad_mode {
                PadMode::HotCue => d.hot_cues.map(|c| c.is_some()),
                PadMode::BeatLoop => {
                    std::array::from_fn(|i| d.active_loop.is_some_and(|l| l.beats == LOOP_BEATS[i]))
                }
            }
        };
        out.decks[deck.index()] = DeckLeds {
            play: d.loaded && d.playing && !d.previewing,
            cue,
            headphone_cue: state.mixer.headphone_cue[deck.index()],
            pad_mode,
            pads: lit,
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
        // Pads light on their own notes, which depend on the mode: a mode change resends
        // all eight.
        if let Some(layout) = mapping.pads {
            let status = layout.status[deck.index()];
            let base = layout.base(new.pad_mode);
            let same_mode = old.is_some_and(|o| o.pad_mode == new.pad_mode);
            for (slot, &on) in new.pads.iter().enumerate() {
                let was = old.filter(|_| same_mode).map(|o| o.pads[slot]);
                if was != Some(on) {
                    out.push([status, base + slot as u8, if on { 0x7F } else { 0x00 }]);
                }
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
        let before = desired(&s, &AppValues::default(), 0.0);
        s.decks[1].playing = true;
        let after = desired(&s, &AppValues::default(), 0.0);
        assert!(after.decks[1].play);
        let msgs = messages(&DDJ_FLX2, Some(&before), &after);
        assert!(msgs.contains(&[0x91, 0x0B, 0x7F]), "{msgs:02X?}");
        assert!(!msgs.iter().any(|m| m[0] == 0x90), "deck A unchanged");
    }

    #[test]
    fn cue_led_on_at_cue_blinks_elsewhere() {
        let mut s = state();
        assert!(
            desired(&s, &AppValues::default(), 0.0).decks[0].cue,
            "paused at cue"
        );
        s.decks[0].position = 44_100.0 * 10.0;
        assert!(desired(&s, &AppValues::default(), 0.0).decks[0].cue);
        assert!(
            !desired(&s, &AppValues::default(), 0.3).decks[0].cue,
            "blinks"
        );
        s.decks[0].playing = true;
        assert!(
            !desired(&s, &AppValues::default(), 0.0).decks[0].cue,
            "off while playing"
        );
    }

    #[test]
    fn headphone_cue_led_follows_mixer() {
        let mut s = state();
        s.mixer.headphone_cue[0] = true;
        assert!(desired(&s, &AppValues::default(), 0.0).decks[0].headphone_cue);
        assert!(!desired(&s, &AppValues::default(), 0.0).decks[1].headphone_cue);
    }

    #[test]
    fn resync_sends_every_led_and_diff_sends_nothing_when_unchanged() {
        let mut s = state();
        s.decks[0].playing = true;
        let leds = desired(&s, &AppValues::default(), 0.0);
        let full = messages(&DDJ_FLX2, None, &leds);
        assert_eq!(full.len(), 6 + 16, "3 button LEDs and 8 pads per deck");
        assert!(
            full.contains(&[0x90, 0x0B, 0x7F]),
            "play A lit after reconnect"
        );
        assert!(messages(&DDJ_FLX2, Some(&leds), &leds).is_empty());
    }

    #[test]
    fn hot_cue_pads_light_when_set() {
        let mut s = state();
        s.decks[0].hot_cues[0] = Some(1_000.0);
        s.decks[0].hot_cues[3] = Some(2_000.0);
        let leds = desired(&s, &AppValues::default(), 0.0);
        let lit: Vec<bool> = leds.decks[0].pads.to_vec();
        assert_eq!(lit, [true, false, false, true, false, false, false, false]);
        // Hot Cue notes on deck A's pad channel.
        let full = messages(&DDJ_FLX2, None, &leds);
        assert!(full.contains(&[0x97, 0x00, 0x7F]));
        assert!(full.contains(&[0x97, 0x03, 0x7F]));
        assert!(full.contains(&[0x97, 0x01, 0x00]));
    }

    #[test]
    fn active_loop_pad_lights_in_beat_loop_mode_and_resyncs() {
        let mut s = state();
        s.decks[1].active_loop = Some(djk_core::ActiveLoop {
            start: 0.0,
            end: 1_000.0,
            beats: 2.0,
        });
        let values = AppValues::default();
        values.set_pad_mode(DeckId::B, PadMode::BeatLoop);
        let leds = desired(&s, &values, 0.0);
        assert!(leds.decks[1].pads[3], "pad 4 = 2 beats");
        assert_eq!(leds.decks[1].pads.iter().filter(|&&p| p).count(), 1);
        // After a reconnect (full resync) the loop pad is lit, on the Beat Loop notes.
        let full = messages(&DDJ_FLX2, None, &leds);
        assert!(full.contains(&[0x99, 0x63, 0x7F]), "{full:02X?}");
    }

    #[test]
    fn mode_selection_lights_the_available_modes() {
        let s = state();
        let values = AppValues::default();
        values.set_selecting(DeckId::A, true);
        let leds = desired(&s, &values, 0.0);
        assert_eq!(
            leds.decks[0].pads,
            [true, false, true, false, false, false, false, false]
        );
    }

    #[test]
    fn mode_change_resends_every_pad() {
        let s = state();
        let values = AppValues::default();
        let before = desired(&s, &values, 0.0);
        values.set_pad_mode(DeckId::A, PadMode::BeatLoop);
        let after = desired(&s, &values, 0.0);
        let msgs = messages(&DDJ_FLX2, Some(&before), &after);
        let pads: Vec<_> = msgs.iter().filter(|m| m[0] == 0x97).collect();
        assert_eq!(pads.len(), 8);
        assert!(pads.iter().all(|m| (0x60..0x68).contains(&m[1])));
    }
}
