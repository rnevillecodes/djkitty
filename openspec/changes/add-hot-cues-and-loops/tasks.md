# Tasks

## 1. Hardware discovery

- [ ] 1.1 With `--midi-monitor`, record the DDJ-FLX2's SHIFT, BEAT SYNC (both decks, with and without SHIFT) and all 8 pads per deck in Hot Cue and Beat Loop modes (plain and with SHIFT), noting whether pad note numbers change with the mode. Verify by adding a "Pads and pad modes" table to `docs/controllers/ddj-flx2.md` that answers whether the hardware tracks the mode itself
- [ ] 1.2 With the monitor's send feature, find the pad LED messages for each mode, and whether software can switch the FLX2's pad mode (the design's open question). Verify each pad LED lights on command, and record the messages and the answer in `docs/controllers/ddj-flx2.md`

## 2. Engine: hot cues

- [ ] 2.1 Add `SetHotCues`, `HotCuePress`, `HotCueRelease` and `ClearHotCue` to `Command`, and `hot_cues: [Option<f64>; 8]` to `DeckState` (still `Copy`). Verify `cargo build --workspace` succeeds and the `no_alloc` test still passes
- [ ] 2.2 Implement hot cues in `Deck` (design Decision 1): set at the playhead when empty, jump while playing, preview while held when paused (release returns and pauses, play during the hold keeps playing), clear, nothing with no track, and slots reset on load. Verify with unit tests covering each `hot-cues` spec scenario for set, jump, preview and clear

## 3. Engine: loops

- [ ] 3.1 Add `SetBeatLength`, `BeatLoop` and `ExitLoop` to `Command`, and `loop_range: Option<(f64, f64)>` to `DeckState`. Implement start, resize, exit-on-same-length, clamping to the track end, refusal with no beat length, and ending the loop when a seek, cue or hot cue moves the playhead outside it. Verify with unit tests for each `loops` spec scenario except seamlessness
- [ ] 3.2 Wrap the read head at the loop end inside `Deck::render`, keeping the leftover, with a ~3 ms equal-power crossfade and no allocation (design Decision 3). Verify with unit tests that a looped sine has no sample-to-sample jump above the normal maximum at the wrap, that 1000 iterations of a 1/4-beat loop at 174 BPM end exactly where expected (no drift), that the reported position never exceeds the loop end, and that `no_alloc` still passes

## 4. Library: saved hot cues

- [ ] 4.1 Add migration v6 creating `hot_cues(track_id REFERENCES tracks ON DELETE CASCADE, slot 0–7, position_secs, UNIQUE(track_id, slot))`, plus `hot_cues(id) -> [Option<f64>; 8]` and `set_hot_cues(id, [Option<f64>; 8])` in seconds. Verify with library tests that cues round-trip, that clearing a slot removes it, that removing a track removes its cues, and that a v5 database migrates

## 5. App: wiring, on-screen pads and waveforms

- [ ] 5.1 On load, read the track's hot cues and send `SetHotCues`, and send `SetBeatLength` from the grid BPM (in track frames) on load and on every grid change. Each frame, diff the snapshot's hot cues against the last saved ones for the current `track_token` and save changes (design Decision 2). Verify by setting cues, restarting, reloading the track and seeing them restored (`hot-cues` spec "Restored after restart"), and by checking that loading a new track never writes the old track's cues
- [ ] 5.2 Add the per-deck pad row with a Hot Cue / Loop switch (design Decision 6): click to set or jump, hold to preview while paused, right-click or Shift+click to clear, loop pads lit for the active length and disabled with a tooltip when the track has no BPM. Verify every `hot-cues` and `loops` "on screen" scenario with no controller connected, and screenshot the deck at the minimum window size to check nothing overlaps
- [ ] 5.3 Draw numbered hot cue markers and the shaded active loop region (with clear start and end) on the scrolling waveform and overview in `waveview.rs`. Verify by screenshot that markers sit on their positions, that the loop shading matches what you hear, and that both stay in sync at 0% and +6% pitch
- [ ] 5.4 Show the "Needs a BPM to loop" deck message when a loop is refused. Verify by pressing a loop pad on a track with no BPM, both on screen and from the controller
- [ ] 5.5 Update the README feature list with hot cues, beat loops and the on-screen pads. Verify the README describes them as built

## 6. Controller: pad modes, pads and LEDs

- [ ] 6.1 Add `Shift`, `BeatSync(deck)` and `Pad { deck, mode, slot, shifted }` controls to the mapping with the messages recorded in task 1.1, and per-deck pad mode and selection state plus SHIFT tracking in the `Translator` (design Decision 4). Emit hot cue and loop engine commands, `Action::PadMode` on a mode change, nothing for BEAT SYNC without SHIFT, and nothing for Pad FX or Sampler choices. Verify with unit tests feeding recorded MIDI bytes for every `controller-input` delta scenario, including that deck modes are independent
- [ ] 6.2 Share per-deck pad modes between the UI and the translator. The on-screen switch updates them, and `Action::PadMode` updates the screen. Verify that switching on the hardware flips the on-screen row, and that the on-screen switch changes what the hardware pads do (or, if task 1.2 found the hardware owns the mode, that this limitation is documented in `docs/controllers/ddj-flx2.md`)
- [ ] 6.3 Add pad LEDs to `LedState` and `led.rs` (design Decision 5): hot cue pads lit when set, the active loop length lit, available modes lit during selection, and resync on connect. Verify with unit tests for each pad LED scenario, and on hardware including a reconnect during an active loop
- [ ] 6.4 Update the control → action table in `docs/controllers/ddj-flx2.md` with SHIFT, BEAT SYNC, pad mode selection and pads per mode. Verify every listed pad action works on the FLX2 as documented

## 7. End-to-end check

- [ ] 7.1 On the DDJ-FLX2, set hot cues on a real track, mix into it using hot cues, switch the pads to Beat Loop with SHIFT + BEAT SYNC, loop a 4-beat section, halve it to 2 and 1, then exit. Verify there are no clicks at the loop wrap, LEDs are correct throughout, the overrun counter stays at 0, neither deck's tempo changes from BEAT SYNC, and the hot cues are still there after a restart

## Workflow follow-up

- Archive `add-v0-1-walking-skeleton` before archiving this change, so the `controller-input` MODIFIED requirement has a main spec to apply to.
