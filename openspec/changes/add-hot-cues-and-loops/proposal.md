# Proposal

## Why

The DDJ-FLX2's eight performance pads per deck do nothing in djkitty today, so the DJ can't use hot cues or loops, two of the first techniques a learner reaches for after cueing and beatmatching. The FLX2 picks its pad mode with **SHIFT + BEAT SYNC** followed by a pad. djkitty never syncs tempo, so that button combination is free to serve as the pad-mode selector it was designed to be.

## What Changes

- **Hot cues:** eight per deck. Pressing an empty slot sets it at the playhead, pressing a set slot jumps to it, and SHIFT + pad clears it. While a deck is paused, holding a hot cue previews from that point, like the main cue. Hot cues are saved per track and come back when the track is loaded again.
- **Beat loops:** eight loop lengths per deck (1/4, 1/2, 1, 2, 4, 8, 16 and 32 beats), measured from the track's beatgrid tempo. A loop starts exactly at the playhead with no snapping to the grid. Pressing the active length again exits the loop, and pressing a different length resizes the loop from the same start point. A track with no BPM can't be looped.
- **Pad mode selection on the DDJ-FLX2:** SHIFT + BEAT SYNC enters mode selection. Pad 1 then picks Hot Cue and pad 3 picks Beat Loop. Pad 2 (Pad FX) and pad 4 (Sampler) are not supported yet and leave the mode unchanged. BEAT SYNC on its own still does nothing, and each deck keeps its own mode.
- **Pad LEDs:** pads that hold a set hot cue light up, the pad for the active loop length lights up, and the pads show which modes can be chosen during mode selection.
- **On screen:** each deck gets a row of eight pads with a Hot Cue / Loop switch that mirrors the controller's mode, so everything works with no controller connected. Hot cue markers (numbered 1–8) and the active loop region appear on the scrolling and overview waveforms.
- The v0.1 rule that a pad press is ignored is replaced: pads now act in the current mode. FX buttons and BEAT SYNC on its own stay ignored.

## Capabilities

### New Capabilities

- `hot-cues`: per-deck hot cue slots: setting, jumping to, previewing, clearing and saving hot cues, and showing them on screen.
- `loops`: per-deck beat loops: starting, resizing and exiting loops of a set number of beats, seamless looping, and showing the active loop.

### Modified Capabilities

- `controller-input`: adds DDJ-FLX2 pad mode selection (SHIFT + BEAT SYNC), pad actions in Hot Cue and Beat Loop modes, and pad LED feedback. The "Unmapped controls are ignored" requirement changes so that pads are no longer listed as ignored.

## Impact

- `djk-core`: new `Command`s for hot cues and loops. `DeckState` gains the hot cue slots and the active loop.
- `djk-audio` (`deck.rs`): hot cue slots, hot cue preview, loop wrap in the render loop with a short crossfade to avoid clicks, and a per-deck beat length supplied by the UI. Must stay real-time safe, with no allocation.
- `djk-controller`: SHIFT, BEAT SYNC and pad messages in the FLX2 mapping (provisional until recorded with `--midi-monitor`), per-deck pad mode state in the translator, and pad LEDs in `led.rs`.
- `djk-library`: a schema migration that adds a `hot_cues` table, plus load and save APIs.
- `djk-app`: the on-screen pad row and mode switch, hot cue and loop drawing in `waveview.rs`, loading saved hot cues and the beat length onto the deck, and saving hot cue changes.
- `docs/controllers/ddj-flx2.md`: pad, SHIFT, BEAT SYNC and pad LED messages to record.
- Depends on `add-v0-1-walking-skeleton`, which defines `controller-input` and is still in progress on the same branch. Archive that change first so this delta has a main spec to apply to.
