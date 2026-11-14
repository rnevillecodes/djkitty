# Proposal

## Why

djkitty's library is a database in `%APPDATA%` that points at audio files wherever they happen to be. If an original is moved, renamed or cleaned out of Downloads, the track is lost along with its hot cues, grid and place in sets. The library also can't move to another computer or a USB stick. A DJ's library should be safe from what happens to the original files, and it should be portable, so you can carry it to a gig and open it anywhere.

## What Changes

- **BREAKING: Vaults replace the global library.** On first launch, the user creates a vault by entering a name and choosing a parent folder. djkitty creates `<parent>\<name>\` and uses it as the single record of everything imported. They can also open an existing vault. There is no library outside a vault, and the old `%APPDATA%` library is not migrated.
- **Vault layout:** `.djkitty\` holds djkitty's own data (the library database, waveform cache and a `vault.toml` marker), `Catalog\` holds the audio, and `Sets\` holds the sets.
- **BREAKING: Every import is copied into the vault.** There is no opt-out. Each file is copied byte for byte into `Catalog\<Artist>\<original file name>`, with a ` (2)`, ` (3)` suffix only when a different file already has that name. Original files are never modified, moved or deleted, and the copies in the vault are never modified either.
- **BREAKING: Track identity changes from file path to file content.** Every track is identified by a content hash. Importing the same file again, from any location, adds nothing. Library paths are stored relative to the vault, so the vault keeps working when it is moved or opened on another computer.
- **Crash-safe import:** a file appears in the Catalog under its final name only once it is completely copied and verified. An interrupted import never leaves a track pointing at a partial file.
- **BREAKING: Removing a track deletes its copy in the vault, permanently.** This is the only operation that deletes audio, and it can only delete files inside the vault's `Catalog\`. The confirmation dialog says the vault copy and its hot cues and beatgrid will be gone, and that the original file is not affected.
- **Sets are mirrored as playlist files:** each set is written as `Sets\<set name>.m3u8`, with paths relative to the Catalog, and kept up to date as the set changes. The library database stays the source of truth, and the files are a readable, portable copy.
- **Broken links replace "missing":** a track whose Catalog file can't be found shows a broken link icon in the library and in sets, and can't be loaded. **Locate file…** repairs it from any copy of the same file, checked by content hash.
- The machine remembers the last vault and opens it on launch. If that vault isn't available (for example, the USB stick isn't plugged in), the vault picker opens and says why. When a vault is created in a folder synced by OneDrive, Dropbox or iCloud, the user is warned.

## Capabilities

### New Capabilities

- `vault`: creating, opening and remembering vaults, the vault folder layout, what happens when a vault isn't available, and the cloud-sync warning.

### Modified Capabilities

- `track-library`: import copies files into the vault's Catalog (with naming and collision rules), duplicates are detected by content and not by path, the library persists inside the vault, missing files become broken links with Locate file…, and removal permanently deletes the vault copy while leaving originals untouched.
- `set-management`: sets are mirrored as `.m3u8` files in the vault's `Sets\` folder.

## Impact

- `djk-library`: the biggest change. The library opens from a vault root and not a data directory. The schema starts over (tracks get a content hash, a vault-relative path and the path they were imported from). Import changes to hash, then copy, then verify, then rename. `remove_track` deletes the Catalog file, guarded by a check that it lies inside the Catalog. Set changes rewrite the set's `.m3u8`. New relink API. Adds a `blake3` dependency for hashing.
- `djk-app`: a first-run and vault-picker screen in place of opening the library straight away; the per-computer settings file gains the last and recent vaults, with audio device and column settings staying per computer; switching vaults empties both decks; the import, analysis and loader threads open the current vault; a broken link icon (a new SVG asset) in the track list and set views; Locate file… in the row menu and the track details; reworded removal dialog and notices.
- `djk-core` / `djk-audio`: a new `Eject` command that empties a stopped deck, used when switching vaults (the old buffer goes out through the existing retire queue, so nothing is freed on the audio thread).
- `devtools` (`--import-folder`, `--analyse`): use the last opened vault.
- Tests in `crates/djk-library/tests/library.rs` that rely on path identity, the old migrations or "remove leaves files untouched" are rewritten for vault behaviour.
- `README.md`: the library section and the "never edits, moves or deletes your audio files" promise are reworded for vaults.
- Disk use: the Catalog holds a full copy of every imported file.
