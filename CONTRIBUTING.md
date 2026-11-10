# Contributing to djkitty

Thanks for helping out! A few ground rules keep the history readable.

## Commit messages: Conventional Commits

**Every commit uses [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).**

```
<type>(<optional scope>): <short summary in the imperative>

<optional body explaining what and why>

<optional footer(s), e.g. BREAKING CHANGE: ...>
```

### Types

| Type       | Use for                                               |
|------------|-------------------------------------------------------|
| `feat`     | a new user-facing feature                             |
| `fix`      | a bug fix                                             |
| `perf`     | a performance improvement                             |
| `refactor` | code change that neither fixes a bug nor adds a feature |
| `test`     | adding or fixing tests                                |
| `docs`     | documentation only                                    |
| `build`    | build system, dependencies, Cargo manifests           |
| `ci`       | CI configuration                                      |
| `chore`    | maintenance that doesn't fit elsewhere                |
| `style`    | formatting only                                       |

### Scopes

Use the crate (without the `djk-` prefix) or area you touched:

| Scope        | Area                                         |
|--------------|----------------------------------------------|
| `core`       | `djk-core` shared types                      |
| `audio`      | `djk-audio` engine, mixer, device output     |
| `analysis`   | `djk-analysis` decoding, BPM, key, waveforms |
| `library`    | `djk-library` database, import, sets         |
| `controller` | `djk-controller` MIDI and mappings           |
| `app` / `ui` | `djk-app` window, layout, widgets            |
| `openspec`   | planning artifacts under `openspec/`         |

### Examples

```
feat(audio): add full-at-centre crossfader curve
fix(controller): ignore jog touch messages while paused
feat(analysis): detect BPM from spectral flux autocorrelation
test(library): cover re-import deduplication
docs(controller): record DDJ-FLX2 MIDI messages
build: bump egui to 0.37
```

Breaking changes add `!` after the type/scope (`feat(core)!: ...`) or a
`BREAKING CHANGE:` footer.

## Code checks

Before opening a pull request:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Real-time audio rules

Code that runs in the audio callback (`djk-audio`'s `Engine::process` and everything it
calls) must never block, lock, allocate, free memory or do I/O. There is a test that
enforces zero allocations; keep it passing.
