# djkitty

**The DJ app that grows with you.** An open-source DJ application written in Rust: simple
enough to learn on, built to grow into a full replacement for commercial DJ software.

> Status: early development (v0.1 "walking skeleton"). Not yet ready for gigs.

## What v0.1 does

- **Library:** import a music folder (recursively) or **drag and drop files and folders**
  onto the window (dropping while a set is open also adds them to that set); search, and sort by title, artist,
  BPM, key or duration. Missing files are flagged.
- **Analysis in the background:** duration, format, sample rate, channels, bitrate,
  **BPM**, **key** (Camelot and standard notation) and a **3-band waveform** for every
  track. Results are cached, so tracks are analysed once.
- **Sets:** create, rename and delete sets; add tracks; reorder them; see track count and
  total duration.
- **Two decks:** load, play/pause, CDJ-style cue (set, return, hold to preview), ±8% pitch
  fader, jog nudge, elapsed/remaining time, and a **scrolling waveform** plus a
  whole-track overview (click to seek while paused).
- **Mixer:** channel faders, a crossfader, master level, a peak limiter (no harsh
  clipping), and **headphone cue** on 4-channel devices (master on 1–2, headphones on 3–4).
- **Controller:** Pioneer DJ / AlphaTheta **DDJ-FLX2**, with hot-plug, soft takeover and
  LED feedback. See [docs/controllers/ddj-flx2.md](docs/controllers/ddj-flx2.md) (the
  mapping is provisional until it has been verified on hardware).
- **Everything also works with mouse and keyboard**, without a controller.

Supported formats: **MP3, FLAC, WAV, AIFF, M4A/AAC**.

## Building and running

You need a stable Rust toolchain from [rustup](https://rustup.rs/). On Windows you also
need the MSVC C++ build tools (Visual Studio Build Tools with "Desktop development
with C++").

```bash
git clone https://github.com/rnevillecodes/djkitty
cd djkitty
cargo run --release -p djk-app
```

The binary is called `djkitty`. Your library is stored in the platform data directory
(on Windows, `%APPDATA%\djkitty\data`). Set `DJKITTY_DATA_DIR` to use a different
directory, for example a throwaway test library.

### Developer flags

| Flag | What it does |
|------|--------------|
| `--list-audio-devices` | Lists output devices with their channel counts, sample rates and buffer sizes |
| `--test-tone [master\|headphones\|silent]` | Plays a 440 Hz tone for 3 s, then prints latency and overruns |
| `--midi-monitor [filter]` | Logs raw MIDI from matching ports (default `DDJ-FLX2`); type hex bytes to send them |
| `--import-folder <path>` | Imports and analyses a folder without opening the UI |

```bash
cargo run -p djk-app -- --list-audio-devices
```

## Keyboard shortcuts

**Ctrl+click** (or double-click) any slider or fader to reset it to its default: pitch 0%,
crossfader centre, faders and master up.

Shortcuts are ignored while you're typing in a text field. Click **Shortcuts** in the
status bar to see this list in the app.

| Action | Deck A | Deck B |
|--------|--------|--------|
| Play / pause | `Q` | `U` |
| Cue (hold to preview) | `W` | `I` |
| Pitch −0.1% / +0.1% | `E` / `R` | `O` / `P` |
| Nudge back / forward (hold) | `D` / `F` | `J` / `K` |
| Headphone cue | `1` | `2` |

| Keys | Action |
|------|--------|
| `↑` / `↓` | Move the track selection |
| `Shift+←` / `Shift+→` | Load the selected track to deck A / B |
| `Ctrl+F` | Search |

## Not in v0.1

Coming later, roughly in this order of priority: key-lock (time-stretch), sync and
beatgrids, loops and hot cues, EQ and filters, effects, scratching (jog touch), stems,
recording, learning hints, streaming services, and other controllers.

## Project layout

| Crate | Purpose |
|-------|---------|
| `djk-core` | Shared types: commands, engine snapshots, BPM and key |
| `djk-audio` | Real-time engine: decks, mixer, limiter, cpal output |
| `djk-analysis` | Decoding, file stats, BPM, key, waveforms |
| `djk-library` | SQLite library, folder import, sets |
| `djk-controller` | MIDI discovery, mappings, soft takeover, LEDs |
| `djk-app` | The egui application |

Planning documents live in [`openspec/`](openspec/).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All commits use
[Conventional Commits](https://www.conventionalcommits.org/).

## Licence

Dual-licensed under MIT or Apache-2.0, at your option. To be confirmed before v0.1.
