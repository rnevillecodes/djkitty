# Design

## Context

- **Engine:** each deck (`djk-audio/src/deck.rs`) is a fractional read head over a decoded buffer, rendered per frame in `Deck::render` on the audio thread. That thread must not allocate, lock or do I/O. Commands reach the engine on two lock-free queues: one from the UI and one straight from the MIDI callback, so controller input has no UI-frame latency. State comes back as a `Copy` `EngineState` snapshot.
- **Beatgrid:** the grid (`djk_core::Beatgrid { bpm, start_secs, first_downbeat }`) lives in the library and in the app (`DeckUi::grid`). The engine knows nothing about it.
- **Controller:** the controller translates statelessly, apart from 14-bit MSBs and soft takeover (`mapping.rs`, `Translator`). LEDs are a pure `desired(state) → diff → messages` function (`led.rs`). The FLX2 mapping still uses provisional FLX4 numbers.
- **Library:** schema migrations are an append-only list keyed by `PRAGMA user_version` (`schema.rs`, currently v5). User edits live on the track (as with `user_grid_*`) so re-analysis never overwrites them.
- **Rules:** the no-sync and no-snap rule (`beatgrid` spec "No automatic beat sync") applies. Loop and hot cue positions are exactly where the DJ presses.

## Goals / Non-Goals

**Goals:**
- Pads act with the same low latency as play and cue: the controller sends commands straight to the engine, not through the UI.
- Loops are sample-accurate and click-free. Hot cue and loop state is part of the engine snapshot, so the UI, LEDs and waveforms all read one source of truth.
- A pad mode model that works whether the FLX2 tracks the mode itself or leaves it to software (see Decision 4).

**Non-Goals:**
- Manual loop in/out, beat jump, loop roll, saved loops, hot cue colours or names, Pad FX and Sampler modes, and keyboard shortcuts for pads. Each can follow as its own change.
- Quantise or snap options of any kind.

## Decisions

### 1. The engine owns hot cue and loop state

The `Deck` holds `hot_cues: [Option<f64>; 8]` (track frames), the held hot cue for preview, `active_loop: Option<ActiveLoop { start, end, beats }>` and `beat_frames: Option<f64>`. All are fixed-size, so there is no allocation on the audio thread. The track's saved hot cues travel inside `LoadRequest`, because the engine refuses loads onto a playing deck and a separate command could land them on the deck's current track. New commands:

- `HotCuePress { deck, slot }` / `HotCueRelease { deck, slot }` / `ClearHotCue { deck, slot }`
- `SetBeatLength { deck, token, frames: f64 }` (0 = no tempo, keeping `Command` within its 24-byte budget): sent by the UI whenever the loaded track's grid tempo is known or changes; ignored if `token` is not the deck's current track.
- `BeatLoop { deck, beats: f64 }`: start, resize or exit, decided in the engine using the rules in the `loops` spec.
- `ExitLoop(deck)`

`DeckState` gains `hot_cues`, `active_loop`, `beat_frames` and a `loop_refusals` counter (so the UI can explain a refused loop even when the press came from the controller), and stays `Copy`.

*Why:* the controller path has no access to the library or grid. If the engine holds the per-deck beat length and the hot cue slots, a pad press is a single engine command from the MIDI thread, as fast as the cue button. The playhead position at the moment the command is processed is the only correct "where the DJ pressed", and only the engine has it.

*Alternative:* route pads to the UI as `Action`s, like browse and load, and have the UI compute positions. Rejected: up to a frame of extra latency, and the UI's position is an extrapolation, not the true playhead.

### 2. Saving hot cues: the UI watches the snapshot

The engine can't do I/O. Each frame the app compares `DeckState::hot_cues` with what it last saved for that deck's track. When they differ (set or cleared from the pads or the screen), it writes them to the library and records what was saved. Positions are stored in **seconds** (`hot_cues(track_id, slot, position_secs)`, migration v6, with `ON DELETE CASCADE` from `tracks`). Seconds don't depend on the device rate, and they match how grids are stored. On load, the app reads them and puts them in the `LoadRequest`. It ignores snapshots whose `track_token` doesn't match yet, so a load in progress never saves the old track's cues against the new one.

*Alternative:* a dedicated engine→UI event queue. Rejected for now: the snapshot diff is simpler, already exists and can't lose events.

### 3. Loop wrap in the render loop, with a short crossfade

In `render`, when `pos >= loop_end`, the read head moves to `loop_start + (pos - loop_end)`, which keeps the leftover so the timing doesn't drift. To avoid a click, the first ~3 ms after each wrap blends the samples just past the loop end into the samples from the loop start, using an equal-power fade. This needs only a frame counter and the pre-wrap position, with no buffers. The playhead never reports a position past `loop_end`, which keeps the waveform in sync.

A loop is clamped to `[start, track_end]`. Resizing keeps `start` and recomputes `end`. If the playhead is now past the new end, it wraps at once. A seek, main-cue return or hot cue jump to a position outside `[start, end)` clears `active_loop` (the `loops` spec, "Leaving the loop region ends the loop"). Loop length is `beats × beat_frames`, from the grid BPM in track frames. Pitch therefore changes how fast the loop plays, not how many beats it holds.

*Alternative:* wrap only at block boundaries. Rejected: not sample-accurate, and a 1/4-beat loop at 174 BPM is ~86 ms, close to a large audio buffer.

### 4. Pad mode in the translator, mapped by message

FLX2 pads probably send different note numbers per mode, as on the FLX4, where the controller tracks the mode and SHIFT shifts the note numbers too. They may instead send one set of notes, leaving the mode to software. The mapping supports both:

- New `Control` variants: `Shift`, `BeatSync(deck)`, and `Pad { deck, mode: Option<PadMode>, slot, shifted }`. A pad whose mode is fixed by its note uses `Some(mode)`. A mode-agnostic pad uses `None`, and the translator applies the deck's current mode.
- The `Translator` keeps each deck's SHIFT state (a pad is shifted by its own SHIFT note or a held SHIFT). SHIFT + BEAT SYNC toggles that deck's selection. While selecting, pad 1 or 3 sets the mode, any pad ends selection, and the choosing pad's release is ignored. Only a press, never a release, changes the mode.
- Pads emit engine commands directly. The pad modes and selection state live in the shared `Arc<AppValues>` already used for soft takeover (per-deck `AtomicU8` and `AtomicBool`), which the translator writes and the app reads every frame to mirror the mode on screen (`controller-input` spec, "On-screen pads follow the controller mode"). One shared value means no separate mode-change message is needed.

For the reverse direction (on-screen switch → controller), the app writes the same shared value, so mode-agnostic pads follow it at once. If the hardware tracks the mode itself and can't be told to switch, a pad press in the other mode moves the deck back to the hardware's mode, and the doc records that limitation. This is an open question below, not a spec change. The spec's mirroring requirement is met in the controller→screen direction either way.

*Alternative:* keep pad mode only in the UI. Rejected: pad presses would then have to go through the UI (see Decision 1).

### 5. Pad LEDs from the snapshot plus the pad modes

`LedState` gains per-deck `pads: [bool; 8]`, computed from the snapshot (`hot_cues` and `active_loop`, whose `beats` gives the active length) and the pad mode or selection state. Because of that it takes the shared pad-mode state as an input. Pad LED note numbers go in `LedMap` as provisional FLX4 values (the pad's own note number on the pad channel), to be confirmed with the MIDI monitor's send feature.

### 6. On-screen pads

A compact row of eight pads per deck sits under the transport, with a two-way Hot Cue / Loop switch beside it, built with the existing `widgets` look (lit ring = set or active). Right-click (or Shift+click) on a set hot cue pad clears it. A hot cue pad held with the mouse previews like the hardware: press sends `HotCuePress`, release sends `HotCueRelease`. Loop pads are disabled with a tooltip when the track has no BPM. When the controller rejects a loop for the same reason, the deck message ("Needs a BPM to loop") is shown, using the existing `DeckUi::message`.

## Risks / Trade-offs

- [The FLX2's pad, SHIFT, BEAT SYNC and pad LED messages are unknown] → The first task records them with `--midi-monitor`. The mapping is data, so only the table changes. Decision 4 covers both mode models.
- [The loop-wrap crossfade could smear very short loops] → The fade is ~3 ms, under 4% of the shortest common loop (1/4 beat at 174 BPM ≈ 86 ms). A unit test checks for no discontinuity at the wrap and an exact loop period.
- [Snapshot-diff saving writes on every change] → At most one small write per pad press. Writes go through the existing library path off the audio thread.
- [Grid edits during an active loop] → The loop keeps its frames. Only new or resized loops use the new tempo. This is stated in the design, not the spec, as it is an edge case.
- [Archive order] → `controller-input` only exists in `add-v0-1-walking-skeleton`, so that change must be archived first. Otherwise the MODIFIED requirement has no target.

## Migration Plan

Library migration v6 adds `hot_cues` and is additive. Older djkitty versions ignore the table. No rollback is needed beyond leaving the table unused.

## Open Questions

- Can software set the FLX2's pad mode (for the on-screen switch to drive the hardware), or only read it? This only affects the screen→controller direction of mirroring. It is answered by the MIDI recording task.
