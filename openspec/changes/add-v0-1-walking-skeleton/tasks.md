# Tasks

## 1. Workspace and hardware discovery

- [x] 1.1 Create the Cargo workspace with crates `djk-core`, `djk-audio`, `djk-analysis`, `djk-library`, `djk-controller` and `djk-app` (binary) as in design §1, plus `.gitignore`, `rustfmt.toml` and a README stub; verify `cargo build --workspace` and `cargo clippy --workspace -- -D warnings` succeed
- [x] 1.2 Add `CONTRIBUTING.md` stating that all commits use Conventional Commits (with examples per crate scope, e.g. `feat(audio): …`); verify the file exists and the first commit follows it
- [ ] 1.3 Add a `--list-audio-devices` dev flag to `djk-app` that prints every cpal output device with its supported channel counts, sample rates and buffer-size range; verify by running it with the DDJ-FLX2 connected and record whether it exposes 4 output channels in `docs/controllers/ddj-flx2.md`
- [ ] 1.4 Add a `--midi-monitor` dev flag that lists MIDI ports and logs raw incoming messages from the DDJ-FLX2 (hex + decoded status/channel/data); verify by moving every v0.1 control and recording each control's message (including 14-bit fader MSB/LSB pairs and jog/browse encoder values) in `docs/controllers/ddj-flx2.md`
- [ ] 1.5 Extend the MIDI monitor with an option to send a note/CC to the controller, and use it to discover the play, cue and headphone-cue LED messages; verify each LED lights on command and record the messages in `docs/controllers/ddj-flx2.md`

## 2. Core types and offline engine

- [x] 2.1 Define shared types in `djk-core`: `DeckId`, `TrackId`, `Command` enum (load buffer, play/pause, cue press/release, seek, set pitch, jog delta, channel fader, crossfader, master/headphone level, headphone cue toggle) and `EngineState` snapshot (per-deck position, playing, cue point, pitch, track rate; audio clock timestamp; overrun count); verify with a unit test that `Command` is `Copy` and the crate builds without std-heavy deps
- [x] 2.2 Implement `Deck` in `djk-audio`: fractional read head over `Arc<[f32]>` with 4-point Hermite interpolation and `step = (track_rate/device_rate)*(1+pitch)*(1+nudge)`, play/pause, end-of-track stop, seek; verify with render tests (a sine at 1 kHz resampled 44.1→48 kHz keeps its frequency; +2.4% pitch shortens playback by 2.4%; playback stops at end)
- [x] 2.3 Implement CDJ-style cue behaviour (set when paused, return-and-pause when playing, hold-to-preview at cue point) and jog behaviour (nudge while playing, seek while paused); verify with render tests for every scenario in the `deck-playback` spec's Cue and Jog nudge requirements
- [x] 2.4 Implement the `Mixer`: squared channel-fader curve, full-at-centre crossfader, master gain, headphone bus (pre-fader cue sum, or master when no cue selected), and a peak limiter with a −0.3 dBFS ceiling on both buses; verify with render tests covering every `audio-mixing` scenario for faders, crossfader, headphone cue and clipping protection (two full-scale signals never exceed the ceiling)
- [x] 2.5 Implement `Engine::process(out: &mut [f32], channels)` that drains both command queues (`rtrb`), renders decks through the mixer, routes master to channels 1–2 and headphones to 3–4 (master-only for 2 channels), publishes an `EngineState` via a triple buffer and pushes replaced buffers onto a retire queue; verify with a test that runs `process` under an allocation-counting global allocator and asserts zero allocations/frees after warm-up
- [x] 2.6 Implement the per-deck 3-band isolator EQ (design §5: Linkwitz-Riley crossovers at 250 Hz / 4 kHz summing flat, knob curve from full kill to +6 dB, ~10 ms gain smoothing) before the channel fader, with a `SetEq` command and EQ values in `EngineState`; verify with tests that centred knobs are flat (±0.1 dB at 50 Hz–10 kHz), each band's kill cuts it by >40 dB while the others stay within 1 dB, full boost adds 6 dB, snapping a knob produces no click, a kill works through the engine, and the zero-allocation test still passes

## 3. Walking skeleton: decode, audio out, minimal window

- [x] 3.1 Implement full-track decoding in `djk-analysis` with symphonia (MP3, FLAC, WAV, AIFF, M4A/AAC) to interleaved stereo `f32` at native rate (mono duplicated, >2 channels downmixed), returning a typed error for undecodable files; verify with tests on small fixture files of each format and on a corrupt file
- [ ] 3.2 Implement cpal output in `djk-audio`: device selection (prefer name containing "DDJ-FLX2", then saved choice, then default), smallest supported buffer size, 4-channel routing when available, stream error → disconnected flag; verify by playing a test tone through the DDJ-FLX2 master output, then the headphone output, and by unplugging during playback without a crash
- [ ] 3.3 Create the minimal eframe app in `djk-app`: open a file dialog, decode on a loader thread, send `LoadBuffer` to deck A, show Play/Pause and Cue buttons, elapsed/remaining time from the snapshot, and drop retired buffers each frame; verify by loading an MP3 and hearing it play/pause/cue through the DDJ-FLX2
- [ ] 3.4 Add the measured output latency (buffer size + device latency) and overrun count to a small status bar; verify the latency shown is ≤ 20 ms on the DDJ-FLX2 and overruns stay at 0 during 5 minutes of playback

## 4. Controller input (DDJ-FLX2)

- [ ] 4.1 Implement MIDI port discovery and hot-plug in `djk-controller` (1 s poll for ports containing "DDJ-FLX2", connect/disconnect events, connection status exposed to the UI); verify by starting with the controller connected, unplugging and replugging, and seeing status update within 3 s with no crash
- [ ] 4.2 Implement the static DDJ-FLX2 mapping table and the `translate` function (buttons, 14-bit absolute faders, relative encoders) for play, cue, pitch, jog, channel faders, crossfader, headphone cue, browse and load, using the messages recorded in task 1.4; verify with unit tests feeding recorded MIDI bytes and asserting the resulting `Command`s, including that pad/FX/sync messages produce no command
- [x] 4.3 Implement soft takeover for channel faders, crossfader and pitch faders; verify with unit tests for the `controller-input` soft-takeover scenario (hardware at 100%, app at 20% → ignored until crossing 20%)
- [ ] 4.4 Route translated commands from the midir callback straight to the engine's controller queue, and send browse/load actions to the UI via a channel; verify play/cue/pitch/jog/faders/crossfader/headphone cue on the hardware drive the engine with no perceptible lag
- [ ] 4.5 Implement LED feedback (diff snapshot vs last-sent state each frame; play lit while playing; cue lit at cue point / blinking when paused elsewhere; headphone cue LEDs) and full resync on connect; verify the `controller-input` LED scenarios on hardware, including reconnect while playing
- [ ] 4.6 Document the final DDJ-FLX2 mapping (control → action table) in `docs/controllers/ddj-flx2.md`; verify every v0.1 control in the table works as documented
- [ ] 4.7 Map the DDJ-FLX2's HI/MID/LOW EQ knobs per channel (14-bit, provisional FLX4 numbers until recorded with `--midi-monitor`) to `SetEq` with soft takeover; verify with a unit test that a knob is ignored until it picks up the centre and then drives its band (done), and on hardware that each knob drives its own band with no jump after an on-screen change

## 5. Track library

- [x] 5.1 Implement the SQLite store in `djk-library` (bundled rusqlite, DB in the platform data dir, `PRAGMA user_version` migrations, tables from design §7); verify with tests that a fresh DB migrates and a reopened DB keeps its data
- [x] 5.2 Implement folder import: recursive walk, supported-extension filter, tag reading via lofty with filename fallback, path-unique insert so re-imports add only new files, and an import summary (added/skipped); verify with tests on a temp folder tree covering the `track-library` import, unsupported-file, re-import and untagged-file scenarios
- [x] 5.3 Implement library queries: list with title/artist/BPM/key/duration, sort by each column (unknown BPM last), and case-insensitive search on title/artist; verify with tests for the search and sort scenarios
- [x] 5.4 Implement missing-file detection (stat on view and at load time, `missing` flag, refusal to load); verify with a test that deletes a file and asserts the track is flagged and cannot be loaded

## 6. Track analysis

- [x] 6.1 Implement file stats extraction (duration, format, sample rate, channels, bitrate or "lossless", VBR via size/duration); verify with tests on fixture files of each format
- [x] 6.2 Implement 3-band waveform generation (biquad split at 250 Hz / 4 kHz, per-band peak at 150 bins/s, `u8` binary cache file per track, overview derived at ~1000 bins); verify with tests that a pure 60 Hz tone lands in the low band, a 8 kHz tone in the high band, and that bin count matches duration × 150
- [x] 6.3 Implement BPM detection (spectral-flux onset envelope via rustfft, autocorrelation over 60–200 BPM with tempo prior, unknown on weak peaks); verify with tests on generated click tracks at 90, 120, 128 and 174 BPM (±1 BPM or half/double) and on noise (unknown)
- [x] 6.4 Implement key detection (chromagram 50 Hz–5 kHz, Krumhansl–Schmuckler correlation, confidence threshold → unknown, Camelot + standard names); verify with tests on synthetic A-minor and C-major chord progressions and on noise (unknown)
- [ ] 6.5 Implement the background analysis pool (`num_cpus - 2`, min 1, below-normal priority) that analyses un-analysed or changed tracks (size/mtime differ), stores results and waveforms, and reports progress; verify with tests that a restart does not re-analyse and a replaced file is re-analysed, and on hardware that 200 tracks analysing during playback cause no overruns
- [x] 6.6 Measure integrated loudness (design §6: BS.1770 K-weighting, gated 400 ms blocks), store it (schema v5, analysis version 5) and show it as a sortable LUFS column and in the track details; verify with tests that the K-weighting matches the 48 kHz coefficients in BS.1770, a full-scale 1 kHz stereo tone reads 0 LUFS (±0.1) at 44.1, 48 and 96 kHz, halving the level drops 6 dB, silence and quiet sections are gated out, and LUFS sorts and round-trips through the library

## 7. Sets

- [x] 7.1 Implement set operations in `djk-library`: create, rename, delete (tracks remain), add track (no duplicates within a set), remove track, reorder with persisted positions, and set summary (count, total duration); verify with tests covering every `set-management` scenario including the 5:00 + 6:30 + 4:15 = 15:45 total

## 8. Full UI

- [x] 8.1 Build the main layout in `djk-app` (two deck panels, mixer strip, sidebar with Library/Sets, track table) with a custom dark theme and per-deck accent colours; verify by screenshot that the layout matches design §9 and works at 1280×720
- [ ] 8.2 Implement deck panels: title/artist/BPM/key, elapsed/remaining, play/pause/cue buttons, pitch slider with BPM and % readout, loading indicator, refusal message when loading onto a playing deck; verify every on-screen deck action works with no controller connected
- [ ] 8.3 Implement the mixer strip (channel faders, crossfader, headphone cue toggles, master and headphone level, "headphone cue unavailable" on 2-channel devices) and the audio device picker with remembered choice; verify on-screen controls change the audio and the device choice survives restart
- [ ] 8.4 Implement the library and set views: import-folder button with progress and summary, search box, sortable columns, missing-track styling, analysis progress, set create/rename/delete, add-to-set, drag or button reorder, load to deck A/B, and browse-knob selection highlight; verify each `track-library` and `set-management` UI scenario by hand
- [ ] 8.5 Implement the scrolling deck waveform (mesh from detail bins in an ~8 s window, band-mixed colours, fixed playhead, position extrapolated from snapshot timestamp and compensated for output latency) with continuous repaint only while playing; verify that it scrolls smoothly at ≥ 60 fps with both decks playing and stays visually in sync with the kick drum at 0% and +6% pitch
- [ ] 8.6 Implement the overview waveform per deck (position and cue markers, click-to-seek when paused) and the track details view (file stats, BPM, key in both notations, small overview); verify the `waveform-display` overview and details scenarios
- [ ] 8.7 Add keyboard shortcuts for core deck actions and list them in the README; verify each documented shortcut works
- [ ] 8.8 Add a HI/MID/LOW EQ knob column beside each channel fader in the mixer strip (arcs in the waveform band colours growing from centre, red at full kill, centre snap, double-click/Ctrl+click reset, dB on hover); verify by screenshot (done) and by ear that each on-screen knob changes its band of the deck's sound

## 9. End-to-end verification

- [ ] 9.1 Run a 30-minute mixing session on the DDJ-FLX2: import a real folder, build a set, mix through it using only the controller with headphone cue; verify zero overruns, no crashes, LEDs always correct, and waveforms in sync throughout
- [ ] 9.2 Run the no-controller path: unplug the DDJ-FLX2, use laptop speakers, and complete load/play/cue/pitch/fader/crossfader with mouse and keyboard; verify everything works and headphone cue shows as unavailable
- [x] 9.3 Update the README with build/run instructions, supported formats, the DDJ-FLX2 doc link and the v0.1 out-of-scope list; verify a fresh clone builds and runs by following it

## 10. Beatgrids

- [x] 10.1 Add the `Beatgrid` model and grid maths to `djk-core` (first beat, bar phase with pickup bars, beat times, bar.beat, set downbeat without moving the start, nudge, tempo change around a pivot keeping the start); verify with unit tests for each
- [x] 10.2 Implement beatgrid detection in `djk-analysis` (design §11: low-passed onset phase histogram, coarse and fine period tuning, calibrated onset delay, grid starting at the first audible beat as bar 1 beat 1); verify with tests that the grid starts within 6 ms of where the music starts at 128 and 174 BPM, a 5-second silent lead-in gets no beats, quiet hiss is not mistaken for the start, and a 5-minute track stays within 10 ms at the end
- [x] 10.3 Store detected and user-edited grids (tempo, first beat, bar phase) in the library (schema v4, analysis version 4, edited grid on the track and winning, reset, edited tempo shown as BPM); verify with a test that an edit wins, survives re-analysis and resets to the latest detected grid
- [x] 10.4 Draw beat lines, bar markers with numbers and phrase highlights in the scrolling lanes, phrase ticks in the overviews, and a bar.beat readout per deck; verify by screenshot on real tracks that bar lines sit on the kicks
- [x] 10.5 Add the GRID chip and editing tools in each lane (Set 1 here with transient snap, Earlier/Later, ÷2/×2, exact tempo, Reset, drag to slide); verify on screen that each tool moves the grid as described and that reset restores the detected grid
- [ ] 10.6 Hands-on check: on real tracks, play with the grid visible and confirm by ear that markers pass the playhead on the beat, and correct one grid with Set 1 here and a nudge
- Beat sync is deliberately never implemented (see the `beatgrid` spec's "No automatic beat sync" requirement).

## Workflow follow-up

- Archive the change with `/opsx:archive` once all tasks are done and reviewed.
