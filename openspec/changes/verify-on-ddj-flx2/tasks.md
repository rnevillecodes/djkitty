# Tasks

Each task names where it came from: **v0.1** = `add-v0-1-walking-skeleton`, **cues** = `add-hot-cues-and-loops`, **filter** = `add-channel-filter` (all archived; their code is built).

## 1. Hardware discovery

- [ ] 1.1 Run `--list-audio-devices` with the DDJ-FLX2 connected and record its device name, output channel count (4 expected), sample rates and buffer-size range in the audio interface table of `docs/controllers/ddj-flx2.md` (v0.1 1.3)
- [ ] 1.2 With `--midi-monitor`, move every v0.1 control (play, cue, pitch, jog side/top/touch, channel faders, EQ knobs, crossfader, headphone cue, browse, load) and record each message, including 14-bit MSB/LSB pairs and encoder values, in the incoming messages table (v0.1 1.4)
- [ ] 1.3 With the monitor's send option, find the play, cue and headphone cue LED messages and record them; each LED must light on command (v0.1 1.5)
- [ ] 1.4 Record SHIFT, BEAT SYNC (both decks, with and without SHIFT) and all 8 pads per deck in Hot Cue and Beat Loop modes, plain and with SHIFT, and answer whether pad notes change with the mode (cues 1.1)
- [ ] 1.5 Find the pad LED messages for each mode, and whether software can switch the FLX2's pad mode (cues 1.2)
- [ ] 1.6 Record both CFX knobs (MSB/LSB, full travel, the centre detent value) and the CFX button, and note which side the hardware labels low-pass (filter 1.1)
- [ ] 1.7 Check whether the FLX2 answers the initial position SysEx with every control's position, and record it in the "Initial control positions" table

## 2. Apply the recordings

- [ ] 2.1 Replace the provisional numbers in the DDJ-FLX2 mapping table and LED messages with the recorded ones (v0.1 controls and EQ, SHIFT, BEAT SYNC, pads, CFX knobs). Change the mapping unit tests to feed the recorded bytes for every `controller-input` scenario, including that pad FX, sampler and CFX button messages yield nothing. Verify `cargo test --workspace` passes (v0.1 4.2, 4.7; cues 6.1; filter 3.1)
- [ ] 2.2 If task 1.5 found the hardware owns the pad mode, document that the on-screen switch cannot change the FLX2's pads (and add a spec delta if behaviour differs); otherwise verify the on-screen switch changes what the hardware pads do and that switching on the hardware flips the on-screen row (cues 6.2)
- [ ] 2.3 Finish the control → action table in `docs/controllers/ddj-flx2.md` (v0.1 controls, EQ, CFX, SHIFT, BEAT SYNC, pad mode selection, pads per mode) and stop calling the mapping provisional in the README. Verify every listed control does what the table says (v0.1 4.6; cues 6.4)

## 3. Hardware checks

- [ ] 3.1 Play a test tone through the FLX2's master output, then its headphone output, and unplug it during playback: no crash, and the app shows it disconnected (v0.1 3.2)
- [ ] 3.2 Load an MP3 and hear play, pause and cue through the FLX2 (v0.1 3.3)
- [ ] 3.3 On the FLX2 audio device, the status bar latency is ≤ 20 ms and overruns stay at 0 over 5 minutes of playback (v0.1 3.4)
- [ ] 3.4 Start with the controller connected, unplug and replug it: the status updates within 3 s with no crash (v0.1 4.1)
- [ ] 3.5 Play, cue, pitch, jog, channel faders, crossfader, headphone cue, browse and load on the hardware drive the app with no perceptible lag (v0.1 4.4)
- [ ] 3.6 Check every `controller-input` LED scenario: play, cue (lit at the cue point, blinking when paused elsewhere), headphone cue, hot cue pads, the active loop length and mode selection, including a reconnect while playing and during an active loop (v0.1 4.5; cues 6.3)
- [ ] 3.7 Each hardware EQ knob drives its own band, and each CFX knob its deck's filter, with no jump after the value was changed on screen (v0.1 4.7; filter 3.1)
- [ ] 3.8 Analysing about 200 tracks while both decks play causes no overruns (v0.1 6.5)
- [ ] 3.9 Pressing a loop pad on the controller for a track with no BPM shows "Needs a BPM to loop" (cues 5.4)

## 4. Listening checks

- [ ] 4.1 Every on-screen mixer control (channel faders, crossfader, master and headphone level, headphone cue) changes the audio as expected, and the chosen audio device is still selected after a restart (v0.1 8.3)
- [ ] 4.2 With both decks playing, the scrolling waveforms run smoothly at 60 fps or more and stay in sync with the kick drum at 0% and +6% pitch (v0.1 8.5)
- [ ] 4.3 Hot cue markers sit where the cues sound, and the loop shading matches what you hear, at 0% and +6% pitch (cues 5.3)
- [ ] 4.4 Each on-screen EQ knob changes only its own band of the sound (v0.1 8.8)
- [ ] 4.5 On real tracks, beatgrid markers pass the playhead on the beat; correct one grid with Set 1 here and a nudge (v0.1 10.6)

## 5. Remaining no-controller checks

Already checked on 2026-11-12: track title, BPM and key; elapsed and remaining time; the play and cue buttons; the pitch readout; the refusal to load onto a playing deck; hot cue set, jump and markers; click-to-seek on the overview while paused; 4-beat loop lit and exited by right-click; hot cues restored after a restart; search and sorting by column; shortcuts Q, E, R, Ctrl+F and Ctrl+←.

- [ ] 5.1 Decks: the loading indicator while a track decodes, and every `waveform-display` track details scenario (file stats, BPM, key in both notations, small overview) (v0.1 8.2, 8.6)
- [ ] 5.2 Library and sets: importing a folder with progress and a summary, missing-track styling, analysis progress, creating, renaming and deleting a set, adding to a set, reordering by drag and by button, and loading to deck A/B, as in each `track-library` and `set-management` UI scenario (v0.1 8.4)
- [ ] 5.3 Every shortcut listed in the README does what it says (v0.1 8.7)
- [ ] 5.4 Pads: preview a hot cue while paused (release returns), Shift+click clears, loop pads are disabled with a tooltip on a track with no BPM and show "Needs a BPM to loop" when pressed, and nothing overlaps at the minimum window size (cues 5.2, 5.4)

## 6. End-to-end sessions

- [ ] 6.1 A 30-minute session on the DDJ-FLX2: import a real folder, build a set, mix through it using only the controller with headphone cue. Zero overruns, no crashes, LEDs always correct, waveforms in sync (v0.1 9.1)
- [ ] 6.2 With the FLX2 unplugged and laptop speakers: load, play, cue, pitch, faders and crossfader with mouse and keyboard all work, and headphone cue shows as unavailable (v0.1 9.2)
- [ ] 6.3 Set hot cues on a real track, mix into it using them, switch the pads to Beat Loop with SHIFT + BEAT SYNC, loop 4 beats, halve to 2 and 1, then exit. No clicks at the wrap, LEDs correct, overruns 0, neither tempo changes from BEAT SYNC, and the cues are still there after a restart (cues 7.1)
- [ ] 6.4 Do a filter transition with the CFX knobs: sweep the outgoing deck to high-pass while bringing the other in, then centre it. No clicks or zipper noise on fast sweeps or across the centre, the centred filter sounds unchanged, the headphones hear it, the screen knob follows the hardware (and picks up correctly after an on-screen reset), overruns 0 (filter 5.1)
