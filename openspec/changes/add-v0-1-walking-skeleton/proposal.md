# Proposal

## Why

djkitty aims to be an open-source DJ app "that grows with you": simple and intuitive for people learning to DJ, with pro features added over time. The repository is empty today. This first change delivers a thin end-to-end slice covering both halves of the product: **prep** (library, analysis, sets) and **play** (two decks driven by a DDJ-FLX2 with realtime waveforms). Doing both together means the shared core (decoding, analysis, waveform data) is designed correctly once, and the riskiest part, low-latency real-time audio with a hardware controller, is proven early.

## What Changes

- Create a new Rust application (Cargo workspace) for Windows first, structured so it can also build on macOS and Linux.
- Import a music folder into a local library and persist it between sessions.
- Analyse each track for duration, format, sample rate, bitrate, BPM and musical key (key is best-effort), and precompute waveform data.
- Create, rename, reorder and delete simple sets (ordered lists of library tracks) and show their total length.
- Two decks, each able to load a track, play/pause, cue (return to cue point), and change tempo with a pitch fader (no key-lock in v0.1).
- A mixer with per-channel volume faders, a crossfader, and headphone pre-listen (cue) routed to a separate output pair.
- Audio out through the DDJ-FLX2's built-in USB sound card: master on one stereo pair, headphones on another.
- DDJ-FLX2 support over USB MIDI: transport buttons, faders, crossfader, pitch, load and browse controls, with LED feedback for play/cue state.
- Realtime UI: a scrolling waveform per deck with a playhead, a full-track overview with position, BPM/key readouts, and a connection status indicator.

Explicitly **out of scope** for v0.1: key-lock/time-stretch, beat sync, beatgrids, scratching/jog-wheel platter behaviour (jogs may only nudge), effects, loops, hot cues, EQ, stems, recording, learning hints, streaming services, and controllers other than the DDJ-FLX2.

## Capabilities

### New Capabilities
- `track-library`: importing folders of audio files, persisting tracks, browsing and searching the library.
- `track-analysis`: extracting file stats (duration, format, sample rate, bitrate), detecting BPM and key, and producing waveform data for display.
- `set-management`: creating and editing sets as ordered, named lists of library tracks.
- `deck-playback`: per-deck track loading, transport (play/pause/cue), tempo adjustment and position reporting.
- `audio-mixing`: combining decks through channel faders and a crossfader into the master output, plus headphone cue routing to a separate output.
- `controller-input`: discovering a supported DJ controller, translating its MIDI messages into app actions, and sending LED feedback (DDJ-FLX2 first).
- `waveform-display`: realtime scrolling deck waveforms, full-track overviews and playhead rendering in the UI.

### Modified Capabilities
<!-- None: this is the first change and no specs exist yet. -->

## Impact

- **New code**: a Rust workspace (engine, analysis, library, controller and UI crates; final layout set in design.md).
- **Dependencies (planned)**: `cpal` (audio I/O, WASAPI on Windows), `midir` (MIDI), `symphonia` (decoding), `rtrb` (lock-free queues), `rusqlite` (library database), `egui`/`eframe` (UI), plus BPM/key analysis code or crates chosen in design.
- **Hardware**: needs a DDJ-FLX2 for end-to-end testing. The app must still run (library, sets, playback to the default output device) without a controller.
- **Process**: all commits follow the Conventional Commits format.
