//! Keyboard shortcuts: the no-controller fallback for core deck actions.
//!
//! Keep `DECK_KEYS`/`GLOBAL_KEYS` in sync with the README.

use djk_controller::FaderTarget;
use djk_core::{Command, DeckId, PITCH_RANGE};
use egui::Key;

use crate::app::DjApp;

/// Pitch change per key press (0.1%).
pub const PITCH_STEP: f32 = 0.001;
/// Jog ticks per second while a nudge key is held.
pub const NUDGE_TICKS_PER_SEC: f32 = 25.0;

struct DeckKeys {
    play: Key,
    cue: Key,
    pitch_down: Key,
    pitch_up: Key,
    nudge_back: Key,
    nudge_fwd: Key,
    headphone: Key,
}

const KEYS: [DeckKeys; 2] = [
    DeckKeys {
        play: Key::Q,
        cue: Key::W,
        pitch_down: Key::E,
        pitch_up: Key::R,
        nudge_back: Key::D,
        nudge_fwd: Key::F,
        headphone: Key::Num1,
    },
    DeckKeys {
        play: Key::U,
        cue: Key::I,
        pitch_down: Key::O,
        pitch_up: Key::P,
        nudge_back: Key::J,
        nudge_fwd: Key::K,
        headphone: Key::Num2,
    },
];

/// (action, deck A key, deck B key) for the shortcuts window and README.
pub const DECK_KEYS: &[(&str, &str, &str)] = &[
    ("Play / pause", "Q", "U"),
    ("Cue (hold to preview)", "W", "I"),
    ("Pitch −0.1% / +0.1%", "E / R", "O / P"),
    ("Nudge back / forward (hold)", "D / F", "J / K"),
    ("Headphone cue", "1", "2"),
];

pub const GLOBAL_KEYS: &[(&str, &str)] = &[
    ("Up / Down", "Move the track selection"),
    (
        "Ctrl+Left / Ctrl+Right",
        "Load the selected track to deck A / B",
    ),
    ("Ctrl+F", "Search the library (or filter the open set)"),
    ("Esc", "Clear the selection (hides track details)"),
    (
        "Del",
        "Remove the selected track from the set, or from the library (files are never deleted)",
    ),
];

pub fn handle(app: &mut DjApp, ctx: &egui::Context) {
    // Never steal keys from a focused text field or an open dialog.
    if ctx.egui_wants_keyboard_input() || app.new_set_dialog || app.remove_prompt.is_some() {
        return;
    }
    let (events, dt) = ctx.input(|i| {
        let mut ev = Vec::new();
        for (deck, k) in DeckId::ALL.into_iter().zip(&KEYS) {
            if i.key_pressed(k.play) {
                ev.push(Ev::Cmd(Command::TogglePlay(deck)));
            }
            if i.key_pressed(k.cue) && !i.modifiers.any() {
                ev.push(Ev::CueDown(deck));
            }
            if i.key_released(k.cue) {
                ev.push(Ev::CueUp(deck));
            }
            if i.key_pressed(k.pitch_down) {
                ev.push(Ev::Pitch(deck, -PITCH_STEP));
            }
            if i.key_pressed(k.pitch_up) {
                ev.push(Ev::Pitch(deck, PITCH_STEP));
            }
            let nudge = i32::from(i.key_down(k.nudge_fwd)) - i32::from(i.key_down(k.nudge_back));
            if nudge != 0 {
                ev.push(Ev::Nudge(deck, nudge as f32));
            }
            if i.key_pressed(k.headphone) {
                ev.push(Ev::Cmd(Command::ToggleHeadphoneCue(deck)));
            }
        }
        if i.key_pressed(Key::ArrowDown) {
            ev.push(Ev::Select(1));
        }
        if i.key_pressed(Key::ArrowUp) {
            ev.push(Ev::Select(-1));
        }
        if i.modifiers.command && i.key_pressed(Key::ArrowLeft) {
            ev.push(Ev::Load(DeckId::A));
        }
        if i.modifiers.command && i.key_pressed(Key::ArrowRight) {
            ev.push(Ev::Load(DeckId::B));
        }
        if i.modifiers.command && i.key_pressed(Key::F) {
            ev.push(Ev::Search);
        }
        if i.key_pressed(Key::Escape) {
            ev.push(Ev::Deselect);
        }
        if i.key_pressed(Key::Delete) {
            ev.push(Ev::Remove);
        }
        (ev, i.stable_dt.min(0.1))
    });

    for ev in events {
        match ev {
            Ev::Cmd(c) => app.audio.send(c),
            Ev::CueDown(d) => {
                let held = &mut app.decks[d.index()].cue_held;
                if !*held {
                    *held = true;
                    app.audio.send(Command::CuePress(d));
                }
            }
            Ev::CueUp(d) => {
                let held = &mut app.decks[d.index()].cue_held;
                if *held {
                    *held = false;
                    app.audio.send(Command::CueRelease(d));
                }
            }
            Ev::Pitch(d, step) => {
                let pitch = (app.audio.state.deck(d).pitch + step).clamp(-PITCH_RANGE, PITCH_RANGE);
                app.audio.send(Command::SetPitch { deck: d, pitch });
                app.ui_fader(FaderTarget::Pitch(d), pitch);
            }
            Ev::Nudge(d, dir) => app.audio.send(Command::Jog {
                deck: d,
                ticks: dir * NUDGE_TICKS_PER_SEC * dt,
            }),
            Ev::Select(delta) => app.move_selection(delta),
            Ev::Load(d) => app.load_selected(d),
            Ev::Search => app.focus_search = true,
            Ev::Deselect => app.selected = None,
            Ev::Remove => app.remove_selected(),
        }
    }
}

enum Ev {
    Cmd(Command),
    CueDown(DeckId),
    CueUp(DeckId),
    Pitch(DeckId, f32),
    Nudge(DeckId, f32),
    Select(i32),
    Load(DeckId),
    Search,
    Deselect,
    Remove,
}
