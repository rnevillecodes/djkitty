# Design

## Context

The repository is greenfield: no code exists yet (see proposal.md for motivation and scope). Constraints that shape the approach:

- **Language: Rust. UI: egui** (decided with the user).
- **Primary platform: Windows 11**, with nothing that blocks macOS/Linux later.
- **Target hardware: DDJ-FLX2.** It's a 2-deck USB MIDI controller with a built-in USB audio interface: master and headphone 3.5 mm outputs, analogue volume knobs. Its sibling, the DDJ-FLX4, is class-compliant and exposes 4 output channels (master 1–2, headphones 3–4). We **assume** the FLX2 behaves the same, but this is unverified (see Risks).
- **Real-time audio rule**: the audio callback must never block, lock, allocate, free or do I/O.
- Solo developer who is learning to DJ, so prefer a few understandable pieces over clever abstractions.

## Goals / Non-Goals

**Goals:**
- A clean separation between the real-time audio thread and everything else, so later features (key-lock, sync, FX, loops) slot in without re-architecture.
- One shared decode/analysis/waveform pipeline used by both prep (library) and play (decks).
- The deck/mixer engine can be tested offline by rendering to a buffer, without an audio device or controller.
- Controller support defined as data (a mapping table), so adding controllers later doesn't touch the engine.

**Non-Goals:**
- A user-facing mapping editor or a scripting language for mappings (v0.1 mappings are built in).
- ASIO support (WASAPI only in v0.1; ASIO can be enabled later via the `cpal` feature).
- Streaming decode for tracks that don't fit in memory.
- Plugin systems, multi-window UI, theming options.

## Decisions

### 1. Cargo workspace with focused crates

```
djkitty/
  Cargo.toml                (workspace)
  crates/
    djk-core/        shared types: TrackId, DeckId, Command, EngineState snapshot, Key, Bpm
    djk-audio/       engine: decks, mixer, limiter, cpal device I/O   (real-time code lives here)
    djk-analysis/    decode (symphonia), file stats, BPM, key, waveform generation
    djk-library/     SQLite store: tracks, analysis results, sets; folder import; tag reading
    djk-controller/  MIDI discovery (midir), mapping tables (DDJ-FLX2), soft takeover, LED output
    djk-app/         egui/eframe binary: UI, wiring, background workers
```

**Why:** each crate maps to one or two capabilities, so they compile and test independently, and `djk-audio` can be held to the real-time rules in isolation. **Alternative:** a single crate with modules. It's simpler at first, but the real-time boundary is easy to blur, and compile times grow quickly with egui and symphonia.

### 2. Threading model and communication

```
  [UI thread / egui] --Command--> rtrb SPSC --+
                                              |
  [MIDI thread / midir] --Command--> rtrb ----+--> [AUDIO CALLBACK (cpal)]
                                                     owns Engine { decks, mixer }
                                                     |
             EngineState snapshot (triple buffer) <--+   positions, playing, cue, levels
             retired buffers --> rtrb --> [UI thread drops them]

  [Loader worker]   decode track -> Arc<[f32]> -> Command::LoadBuffer
  [Analysis pool]   analyse new tracks -> write to library + waveform cache
```

- Commands are small `Copy` enums (`Play(DeckId)`, `SetPitch(DeckId, f32)`, `SetCrossfader(f32)`, …). **Each producer gets its own SPSC queue** (`rtrb`) and the callback drains both at the start of each block. This avoids an MPSC queue's locks or allocations.
- Engine state goes out through a lock-free triple buffer. The UI and controller LED logic read the latest snapshot. The snapshot includes the sample position plus the **audio clock timestamp** of that block, so the UI can extrapolate position between callbacks (see decision 9).
- Track buffers arrive as `Arc<[f32]>` already allocated by the loader. When a deck's buffer is replaced, the old `Arc` is pushed onto a "retire" queue and dropped on the UI thread, so nothing is ever freed on the audio thread.
- The MIDI → engine path skips the UI thread entirely, so controller response doesn't depend on frame rate.

**Alternative:** `Mutex<Engine>` shared between threads. It's simpler, but risks priority inversion and dropouts, which breaks the dropout-free requirement in `audio-mixing`.

### 3. Decks play fully decoded in-memory buffers

On load, the loader thread decodes the whole track with `symphonia` to interleaved stereo `f32` at the file's native sample rate. A 6-minute track is about 127 MB, so two decks fit easily on any modern machine. Playback reads with a fractional read head:

`step = (track_rate / device_rate) * (1 + pitch) * (1 + jog_nudge)`

It uses 4-point Hermite interpolation. One code path covers sample-rate conversion, the pitch fader and jog nudge, and seeking or cueing is just moving an index.

**Alternatives:** streaming decode with a ring buffer saves memory, but makes seeking, cueing and jog much harder, so it's deferred. A dedicated resampler crate (`rubato`) isn't needed, because the variable-rate read already resamples. It can come back when key-lock (time-stretch) arrives in v0.2 and replaces this read path.

### 4. Audio output via cpal on WASAPI

- Use the `cpal` default host (WASAPI shared mode on Windows) and request the smallest buffer size the device supports, aiming for 128–256 frames at 44.1/48 kHz, comfortably inside the 20 ms target.
- **Device choice:** prefer a device whose name contains "DDJ-FLX2", then the user's saved choice, then the system default.
- **Routing:** with 4 or more output channels, the master goes to 1–2 and the headphones to 3–4. With a 2-channel device, master only, and the UI reports headphone cue as unavailable.
- **Disconnect:** a cpal stream error sets a flag and the UI offers to reopen. The engine state survives because the `Engine` is rebuilt from the last snapshot plus the loaded buffers held by the app.

**Alternative:** ASIO has lower latency, but needs the Steinberg SDK at build time and isn't required to meet 20 ms with a class-compliant USB device. It's deferred.

### 5. Mixer maths

- Channel fader: gain = `fader^2` (an approximately logarithmic, natural-feeling curve).
- Crossfader: a standard "full at centre" curve, `gA = min(1, 2*(1-x))` and `gB = min(1, 2*x)` for `x` in [0, 1], applied after the channel faders. This matches the `audio-mixing` spec (centre means both at full level). Selectable curves come later.
- Master: master gain, then a simple peak limiter (fast attack, ~100 ms release) with a hard ceiling at −0.3 dBFS as a final safety net, so the output never wraps.
- Headphones: the sum of cue-enabled decks, taken before the channel fader and crossfader, times the headphone gain. If no deck has cue enabled, the headphones carry the master. The headphone bus also goes through the limiter.
- EQ: a per-deck 3-band isolator, applied right after the deck and before the channel fader (so the channel meter is post-EQ). Each channel splits into low/mid/high with 4th-order Linkwitz-Riley crossovers at 250 Hz and 4 kHz. The low band also passes through the 4 kHz crossover (split and summed) so its phase matches the other bands, which makes the three sum flat when every knob is centred. Knob curve: centre = 0 dB, full right = +6 dB, the lower half covers −26..0 dB and the last few percent fade to a full kill. Gains glide per sample with a ~10 ms one-pole smoother, so knob moves never click. All of it runs in the existing allocation-free render path, driven by a `SetEq { deck, band, value }` command; knob values are published in `EngineState`.

### 6. Analysis pipeline (in-house DSP, permissive dependencies)

All analysis decodes once, mixes down to mono, and resamples to 22,050 Hz for BPM/key (full rate for waveforms):

- **File stats:** from symphonia codec parameters. Bitrate comes from the codec parameters, or is computed as file size / duration for VBR files. Lossless formats show "lossless".
- **BPM:** onset-strength envelope (spectral flux from an STFT via `rustfft`), autocorrelation over lags for 60–200 BPM, with a tempo prior weighted toward 90–140 to resolve half/double. If the autocorrelation peak is too weak, report unknown.
- **Key:** chromagram (12-bin pitch-class profile over the STFT, ~50 Hz–5 kHz), averaged over the track, correlated with Krumhansl–Schmuckler major/minor profiles. If the best correlation minus the runner-up is below a threshold, report unknown. Map to Camelot.
- **Waveform:** split into 3 bands with biquad filters (low < 250 Hz, mid 250 Hz–4 kHz, high > 4 kHz), then store the per-band peak at **150 bins per second** (the detail level). The overview is derived by max-reducing to ~1,000 bins. It's stored as a compact `u8` binary file per track in the app's cache directory, keyed by track ID, which keeps SQLite small.
- Runs on a small worker pool (`num_cpus - 2`, minimum 1) at below-normal thread priority so it doesn't compete with audio.

**Alternatives:** `aubio` (GPL, painful C build on Windows) and `essentia` (C++, AGPL). Both are heavier than this first pass needs. The accuracy targets in `track-analysis` are modest on purpose. Swapping in a better detector later doesn't change the specs.

### 7. Library storage

- `rusqlite` with the `bundled` feature, so there's no system SQLite dependency. The DB lives at `directories::ProjectDirs("", "", "djkitty").data_dir()/library.db`.
- Tables: `tracks(id, path UNIQUE, title, artist, size, mtime, missing)`, `analysis(track_id, duration_ms, format, sample_rate, channels, bitrate, bpm, key, analysed_at)`, `sets(id, name)`, `set_tracks(set_id, track_id, position, UNIQUE(set_id, track_id))`.
- Schema migrations use `PRAGMA user_version`, applied at startup.
- Tags come from `lofty`. The folder walk uses `walkdir` and filters on extension, and the decode check happens during analysis. A file that fails to decode is removed from the import and counted as skipped.
- Missing-file check: a cheap `stat` of visible rows when the library view loads, and always at load time.

### 8. Controller mappings as data

- `midir` opens the input and output ports whose names contain "DDJ-FLX2". A 1-second poll of the port list handles hot-plug, and on connect the app resyncs all LEDs from the current snapshot.
- A mapping is a static table: `(status, data1) -> Control`, where `Control` is a button, a 14-bit absolute value (Pioneer sends faders as MSB/LSB CC pairs) or a relative encoder (jog/browse, centred on 64). A pure function `translate(Control, value, &mut TakeoverState) -> Option<Command>` turns it into engine commands. It's unit-testable with recorded MIDI bytes.
- Soft takeover: track the last hardware value per fader (and per EQ knob, which is also a 14-bit absolute control), and only apply it once the hardware crosses the engine's current value.
- LED output: on each UI frame, compare the snapshot with the last-sent LED state and send only the changes.
- The actual FLX2 message numbers come from AlphaTheta's published MIDI message list if one is available. Otherwise they come from a built-in **MIDI monitor dev mode** (`djkitty --midi-monitor`) that logs raw messages, which is the first task.

**Alternative:** a JS/Lua mapping engine like Mixxx's. It's flexible but heavy for one controller. The table format can be moved to a TOML/RON file later, when user-editable mappings arrive.

### 9. UI (egui/eframe)

- `eframe` with the `wgpu` renderer. A custom dark theme (one accent colour per deck, larger hit targets) supports the "simple and intuitive" goal.
- Layout: two deck panels on top (scrolling waveform, overview, BPM/key, time, transport, pitch), a mixer strip in the middle (channel faders with meters, a HI/MID/LOW EQ knob column beside each channel, crossfader, cue toggles), and a sidebar plus track table at the bottom (Library / Sets).
- **Waveform rendering:** build an egui `Mesh` per deck each frame from the cached detail bins in the visible window (8 s × 150 bins/s = 1,200 columns, cheap), with colours mixed from band levels. Display position = snapshot position + (now − snapshot timestamp) × current rate − output latency, so scrolling is smooth between audio callbacks and matches what you hear.
- Request continuous repaint while any deck is playing, and repaint on input otherwise, to save battery when idle.
- Keyboard shortcuts for the main deck actions as a no-controller fallback (exact keys are set during implementation).

### 10. Cross-cutting

- Logging via `tracing`, with a rolling file in the data directory. Nothing is logged from the audio callback except through an atomic overrun counter.
- `anyhow` in the app crate and `thiserror` in library crates.
- Tests: engine rendering tests (feed commands, render N frames, assert on samples), analysis tests on generated signals (click tracks at known BPM, synthetic chords for key), mapping tests from recorded MIDI, and library tests on a temp DB.
- CI is out of scope here. Commits follow Conventional Commits.

## Risks / Trade-offs

- **[FLX2 audio might not expose 4 channels, or might not be class-compliant]** → Verify first (task 1). Fallback: master on the FLX2 and headphones on a second device is out of scope. Instead, v0.1 ships master-only for that case, which the spec already allows for 2-channel devices.
- **[The FLX2's MIDI messages are undocumented]** → The MIDI monitor dev mode plus a recorded-bytes test fixture. FLX4 mappings in Mixxx are a useful reference for the Pioneer conventions (14-bit faders, jog encoding).
- **[WASAPI shared mode latency exceeds 20 ms on some machines]** → Request the minimum buffer, show the measured latency in settings, and keep WASAPI exclusive mode or ASIO as the next step if needed.
- **[In-memory decode uses ~130 MB per deck and adds a load delay of ~0.5–2 s]** → Acceptable for v0.1. Show a loading indicator on the deck. Streaming can come later.
- **[In-house BPM/key may be inaccurate for some genres]** → Modest spec targets, "unknown" instead of wrong guesses, and the analysis is isolated behind one crate so it can be swapped.
- **[Hermite interpolation adds slight aliasing at large pitch offsets]** → Inaudible at ±8% for a beginner app. Replaced by time-stretch/resampling in v0.2.
- **[egui's default look may feel "tool-like"]** → A dedicated theming task, and keep the layout sparse.
- **[Complex lock-free plumbing for a solo developer]** → Keep the command set small and documented, and centralise all cross-thread types in `djk-core`.

## Open Questions

- **Project licence** (MIT/Apache-2.0 vs GPL-3.0). Every v0.1 dependency is permissive, so this doesn't affect the work. It does matter before adopting Rubber Band (GPL) for key-lock in v0.2.
- **Exact keyboard shortcuts and theme colours.** These are UI polish, settled during implementation.
- **Whether AlphaTheta publishes an FLX2 MIDI message list.** This affects only how we obtain the mapping numbers, not the design.
