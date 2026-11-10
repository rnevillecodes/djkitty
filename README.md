# djkitty

**The DJ app that grows with you.** An open-source DJ application written in Rust: simple
enough to learn on, built to grow into a full replacement for commercial DJ software.

> Status: early development (v0.1 "walking skeleton"). Not yet usable for gigs.

## v0.1 goals

- Import a music folder, see file stats, BPM, key and waveforms
- Build and order sets
- Two decks with realtime scrolling waveforms
- Play on a Pioneer DJ / AlphaTheta **DDJ-FLX2** (or with mouse and keyboard)

## Building

Requires a stable Rust toolchain (`rustup`). On Windows you also need the MSVC C++ build tools.

```bash
cargo run --release -p djk-app
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All commits use
[Conventional Commits](https://www.conventionalcommits.org/).

## Licence

Dual-licensed under MIT or Apache-2.0, at your option (to be confirmed before v0.1).
