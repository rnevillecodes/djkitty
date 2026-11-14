<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="crates/djk-app/assets/logo-wordmark.svg">
    <img src="crates/djk-app/assets/logo-wordmark-light.svg" alt="DJKitty" width="420">
  </picture>
</h1>

**The DJ app that grows with you.** An open-source DJ application written in Rust: simple
enough to learn on, built to grow into a full replacement for commercial DJ software.

> Status: early development (v0.1 "walking skeleton"). Not yet ready for gigs.

## What v0.1 does

- **Vaults:** your library lives in a **vault**, a folder you create on first launch (give it a name and pick
  where it goes: any drive, or a USB stick to take to a gig). Every track you import is **copied** into it, so
  the vault keeps working when the originals move or disappear, and it can be moved, renamed or opened on another
  computer as it is. djkitty reopens the last vault on launch; the vault button in the library toolbar switches
  to another vault or creates a new one (with both decks stopped). You are warned before using a folder synced by
  OneDrive, Dropbox or iCloud Drive, as syncing can damage the library database.

  ```text
  My DJ Vault\
    .djkitty\            library database and waveform cache (djkitty's own)
    Catalog\<artist>\    a copy of every imported file, under its original name
    Sets\<set>.m3u8      each set as a playlist that other players can open
  ```
- **Library:** import files or a music folder (recursively) from the **Import** button, or **drag and drop files and folders**
  onto the window (dropping while a set is open also adds them to that set). Files are copied byte for byte into
  `Catalog\<artist>\`; a file already in the vault (the same content, from anywhere) is not copied again, and a
  different file with the same name gets a ` (2)` suffix. Search, and sort by title, artist,
  BPM, key, LUFS or duration; drag the column dividers to resize (the columns always fill the list). A track whose
  file is missing from the vault shows a **broken link** icon: right-click it and choose **Locate file…** to copy it
  back from any copy of the same file. Remove tracks from a set, or from the library (right-click or `Del`), which
  **permanently deletes the vault's copy** after asking. **djkitty never changes, moves or deletes your original
  files**, and never edits the copies in the vault either: hot cues, grids and sets live in the library.
- **Analysis in the background:** duration, format, sample rate, channels, bitrate,
  **BPM**, **key** (Camelot and standard notation), **loudness** (LUFS, EBU R128) and a **3-band waveform** for every
  track. Results are cached, so tracks are analysed once.
- **Sets:** create, rename and delete sets; add tracks; sort by any column (click **#** for the set order); drag tracks to reorder them; see
  track count and total duration. Each set is kept as an `.m3u8` playlist in the vault's `Sets` folder, rewritten
  from the library whenever the set changes (edits made to the file elsewhere are overwritten).
- **Two decks:** load by dragging a track from the list onto a deck, its waveform or that
  side of the mixer (or with `Ctrl+←` / `Ctrl+→`, or the right-click menu), play/pause, CDJ-style cue (set, return, hold to preview), ±8% pitch
  fader, jog nudge, elapsed/remaining time, and a **scrolling waveform** plus a
  whole-track overview (click to seek while paused).
- **Beatgrids:** CDJ-style beat and bar markers on the waveforms (bar numbers, phrase
  markers every 16 bars) and a bar.beat counter per deck. Detected automatically; click
  **GRID** in a waveform to fix one (set bar 1, nudge, halve/double or type the tempo,
  drag to slide). Your corrections are kept.
- **Hot cues and loops:** eight pads per deck, on screen and on the controller. In **Hot Cue**
  mode a pad sets a hot cue at the playhead, then jumps to it (hold it on a paused deck to
  preview); right-click, Shift+click or SHIFT + pad clears it. Right-click the deck's **CUE**
  switch, or a track in the library, to clear all of a track's hot cues (after asking). Hot
  cues are saved with the track. In **Loop** mode the pads loop 1/4 to 32 beats from exactly
  where you press (never snapped to the grid); press another pad to resize, or the lit pad
  again, SHIFT + any pad or right-click any pad to exit. On the DDJ-FLX2, SHIFT + BEAT SYNC
  then pad 1 or pad 3 picks the mode. Hot cues and the active loop show on the waveforms.
- **Mixer:** channel faders, a **3-band EQ** per channel (HI/MID/LOW, +6 dB boost to full kill), a
  **FILTER** knob per channel (left sweeps a low-pass down, right sweeps a high-pass up, off at
  the centre; on the DDJ-FLX2 it is the CFX knob, with the Smart CFX button unused), a
  crossfader, master level, a peak limiter (no harsh
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

The binary is called `djkitty`. Your library lives in the vault you create or open on
first launch. Per-computer settings (audio device, column widths, the last and recent
vaults) are stored in the platform data directory (on Windows, `%APPDATA%\djkitty\data`).
Set `DJKITTY_DATA_DIR` to use a different settings directory, for example to start fresh
at the welcome screen.

### Developer flags

| Flag | What it does |
|------|--------------|
| `--list-audio-devices` | Lists output devices with their channel counts, sample rates and buffer sizes |
| `--test-tone [master\|headphones\|silent]` | Plays a 440 Hz tone for 3 s, then prints latency and overruns |
| `--midi-monitor [filter]` | Logs raw MIDI from matching ports (default `DDJ-FLX2`); type hex bytes to send them |
| `--import-folder <path>` | Imports (copies) and analyses a folder into the last opened vault without opening the UI |
| `--analyse [paths…]` | Prints BPM, key, loudness and beatgrid for files or folders (no paths: every track in the last opened vault, next to its stored BPM). Never saves results or touches audio files |

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
| `Ctrl+←` / `Ctrl+→` | Load the selected track to deck A / B |
| `Ctrl+F` | Search the library (or filter the open set) |
| `Esc` | Clear the selection (hides the track details panel) |
| `Del` | Remove the selected track from the open set, or from the library (asks first, then deletes the vault's copy; your original file is never touched) |

## Not in v0.1

Coming later, roughly in this order of priority: key-lock (time-stretch), manual loop in/out
and beat jump, filters, effects, scratching (jog touch), stems, recording, learning hints,
streaming services, and other controllers.

**Never:** automatic beat sync. djkitty is for learning to DJ by hand, so beatgrids are a
visual aid and beatmatching stays your job.

## Project layout

| Crate | Purpose |
|-------|---------|
| `djk-core` | Shared types: commands, engine snapshots, BPM and key |
| `djk-audio` | Real-time engine: decks, mixer, limiter, cpal output |
| `djk-analysis` | Decoding, file stats, BPM, key, waveforms |
| `djk-library` | Vaults, SQLite library, import into the Catalog, sets and their playlists |
| `djk-controller` | MIDI discovery, mappings, soft takeover, LEDs |
| `djk-app` | The egui application |

Planning documents live in [`openspec/`](openspec/).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All commits use
[Conventional Commits](https://www.conventionalcommits.org/).

## Licence

Dual-licensed under MIT or Apache-2.0, at your option. To be confirmed before v0.1.
