# Design

## Context

- **Library today** (`djk-library`): `Library::open_in(dir)` opens `dir\library.db` plus `dir\waveforms\`. The app always passes `default_data_dir()` (`%APPDATA%\djkitty\data`, or `DJKITTY_DATA_DIR`). `tracks.path` is an absolute path with `UNIQUE`, and that is the track's identity. Import (`import_paths`) walks paths, probes, reads tags and inserts, all in one transaction. `remove_track` deletes the row and the waveform cache and never touches audio.
- **Who opens the library:** the UI thread (`DjApp::library`), each analysis worker (`analysis.rs`, its own connection), the import thread (`start_import`, its own connection) and devtools. They all receive absolute `PathBuf`s from `TrackRow::path` and `tracks_needing_analysis`.
- **Machine settings:** `settings.txt` (`key=value`) in the data dir, holding the audio device and column widths.
- **Engine:** decks are only replaced by `Load`. The outgoing buffer leaves through a retire queue so the audio thread never frees memory. There is no way to empty a deck.
- **Schema:** seven append-only migrations, including dead columns (`grid_anchor`, `user_grid_anchor`) kept only for databases already upgraded.

## Goals / Non-Goals

**Goals:**
- Every vault-relative path, hash and file operation lives in `djk-library`, so the app, workers and devtools still get absolute paths and need little change.
- No audio-file operation outside the vault can be expressed through the library API: originals are only ever opened for reading.
- Import is restartable at any point: a crash loses at most the file being copied.

**Non-Goals:**
- Migrating the old `%APPDATA%` library. It is left on disk, unread.
- "Find missing files in folder…" (bulk relink), "Verify vault" (re-hash everything) and cleaning orphan Catalog files. The hash column makes all three straightforward follow-ups.
- Reading set changes back from `.m3u8` files, and syncing between vaults.
- Free-space checks before an import (a full disk is handled as a copy failure).
- Hiding `.djkitty` with file attributes.

## Decisions

### 1. A `Vault` owns the folder layout; `Library` opens from a vault root

A new `vault` module in `djk-library`:

```
<root>\
  .djkitty\vault.toml      marker: format = 1
  .djkitty\library.db
  .djkitty\waveforms\<id>.wf
  .djkitty\incoming\       in-progress copies (*.part)
  Catalog\<artist>\<file>
  Sets\<set>.m3u8
```

`Vault::create(parent, name)` validates the name (Decision 5's rules, refusing it if the cleaned name differs from what was typed), refuses an existing target, and creates the folders and the marker. `Vault::open(root)` requires the marker and refuses `format` values newer than it knows ("made by a newer djkitty"). `Library::open(&Vault)` replaces `open_in`. On open it deletes `.djkitty\incoming\*` and rewrites set files (Decision 7).

`TrackRow::path`, `loadable_path` and `tracks_needing_analysis` keep returning **absolute** paths, built as `root.join(relative)`. The loader, analysis and UI code don't change. Relative paths are stored with `/` separators, so a vault made on one OS reads the same on another.

*Why:* this keeps the vault model out of the app. *Alternative:* store the vault root in the database. Rejected, because that would break portability, which is the reason for relative paths.

### 2. Schema restarts at v1

The migration list is replaced by a single v1 that creates the current tables without the dead columns, plus:

- `tracks.hash TEXT NOT NULL UNIQUE`: BLAKE3 hex of the file bytes.
- `tracks.path TEXT NOT NULL UNIQUE COLLATE NOCASE`: Catalog-relative. Case-insensitive, because Windows file names are.
- `tracks.source_path TEXT`, `source_size`, `source_mtime`: where it was imported from (Decision 3's fast path, and a hint in Locate file…).
- `sets.file_name TEXT UNIQUE COLLATE NOCASE`: the playlist file djkitty owns for the set (Decision 7).

`AUTOINCREMENT` ids, `ON DELETE CASCADE`, `hot_cues`, the analysis version and the beatgrid columns stay as they are. `ANALYSIS_VERSION` is unchanged.

*Alternative:* add v8 on top. Rejected: no database needs upgrading (the change is breaking), and carrying seven historic migrations into every new vault is just noise. The migration mechanism stays for future versions.

### 3. Import: hash, then copy and verify, then rename, then insert

For each supported, probe-able file:

1. **Fast path:** if a track has the same `source_path`, `source_size` and `source_mtime`, count it as existing without reading the file. Re-importing a big folder then costs one `stat` per file.
2. **Hash** the source (BLAKE3, streamed). If the hash is in `tracks`, count it as existing.
3. **Copy** into `.djkitty\incoming\<random>.part` while hashing again, then `sync_all`. If the second hash differs, the file changed during import: delete the `.part` and count it as skipped.
4. **Name** it (Decision 5) and **rename** into the Catalog. `incoming` is on the same volume as the Catalog, so the rename is atomic.
5. **Insert** the row and **commit, one file at a time**, so finished files survive a crash. If the insert fails, delete the just-renamed file.

*Why two reads instead of one:* a single pass (copy while hashing, discard if it's a duplicate) writes a full copy for every duplicate. Re-importing folders is common, and reads are much cheaper than writes. *Why per-file commits:* a single transaction around a copy that takes many minutes would lose everything on a crash and hold the write lock throughout, blocking the analysis workers.

A crash between steps 4 and 5 leaves an orphan Catalog file without a row. It is harmless: a re-import would copy again with a ` (2)` suffix, and orphan cleanup is a non-goal. An I/O error that isn't per-file (for example, the disk is full) stops the import, and the summary reports how far it got.

Import still runs on its own thread and reports progress. The counter now advances per file *copied*, and the notice reports bytes as well as files.

### 4. Hashing uses BLAKE3

The `blake3` crate (pure Rust option, SIMD where available) is added as a `djk-library` dependency. It is fast enough that hashing keeps up with disk reads. Collisions are not a practical concern.

*Alternative:* SHA-256 is slower with no benefit here, and xxHash isn't collision-resistant, which matters once the hash is the identity.

### 5. Naming and cleaning names

`sanitize_component(s)` replaces `< > : " / \ | ? *` and control characters with `_`, trims trailing dots and spaces, appends `_` to reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`9`, `LPT1`–`9`, with or without an extension), and caps the result at 100 characters (keeping the extension for file names). An empty result becomes `Unknown Artist` for folders. It is used for artist folders, file names (for sources from other operating systems) and set file names.

When choosing a name, `<stem>.<ext>`, `<stem> (2).<ext>` and so on are tried until one is free. A name counts as taken if a track row has that path (case-insensitive) **or** a file exists there (which catches orphans). The track's displayed artist and title always come from tags, never from the folder name.

### 6. Removing a track deletes the Catalog file, with a guard

`remove_track` deletes the row (cascading to analysis, hot cues and set entries), rewrites the affected sets' playlist files, then calls `delete_catalog_file(rel)`:

- Resolve `root.join(rel)` and `catalog_root` with `canonicalize`, which follows symlinks and `..` and gives `\\?\` paths on both sides.
- Delete only if it is a file and starts with the canonical Catalog root. Otherwise log a warning and return without error (the track is already gone, as the spec requires).
- Then remove the artist folder if it's now empty, under the same check.

Deleting the row first means a failed file delete leaves an orphan, never a row pointing at nothing. Errors from the file delete itself are reported to the user as "Removed from the library, but the file could not be deleted".

`delete_catalog_file` is the only function in the crate that deletes audio. A unit test plants an outside file behind a crafted `path` value (absolute, `..\..\`, and a symlink/junction where the OS allows) and checks it survives.

### 7. Set playlist files, owned by name

After any set change commits, the library rewrites that set's file. It writes `Sets\.<name>.tmp`, then renames it over the old file. The UTF-8 contents are:

```
#EXTM3U
#EXTINF:372,Bicep - Glue
../Catalog/Bicep/02 Glue.mp3
```

Paths are relative to `Sets\`, use `/` separators and are not URL-encoded. That format is accepted by VLC, foobar2000 and most DJ software accept. `sets.file_name` records which file belongs to each set. Rename writes the new file, then deletes the old recorded name. Delete removes only the recorded name. Files djkitty never recorded are never touched. A file name clash between sets gets the ` (2)` suffix.

On vault open, every set file is rewritten from the database. A failure to write a set file is logged and shown as a notice, but never fails the set operation, because the database is the truth.

### 8. Locate file…

`relink(id, picked)` hashes the picked file. If the hash differs from `tracks.hash`, it returns `LibraryError::NotSameFile`. Otherwise it copies the file through the same `incoming` → rename steps to the track's **stored** relative path (recreating the artist folder) and clears `missing`. If a file already exists at that path, it is hashed: if it matches, only the flag is cleared; if not, the relink is refused, because the occupying file isn't ours to replace. The file picker opens in the folder of `source_path` when it still exists.

### 9. Per-computer settings carry the vault list

`settings.txt` gains `last_vault=<abs path>` and `recent_vaults=<path>|<path>…` (the five most recent). It stays in the per-computer data dir (`DJKITTY_DATA_DIR` still overrides it, which tests rely on). Audio device and column widths stay per computer. Nothing in the vault records a computer-specific setting.

### 10. App: welcome screen, switching and ejecting

- `DjApp` gets `vault: Option<Vault>`. Startup tries `last_vault`. On failure it shows the welcome screen with the reason ("not available", "not a vault", "made by a newer djkitty"). While `vault` is `None`, the central area shows the welcome screen (create / open / recent vaults), and import, drag-and-drop and loading are disabled. The decks and mixer still render, so audio-device problems are still visible.
- The create dialog has a name field, a "Choose location…" button (`rfd` folder picker) and a live preview of the full path, with the error under the field.
- **Cloud warning:** the target counts as synced if it is under `%OneDrive%`, `%OneDriveConsumer%` or `%OneDriveCommercial%`, under the Dropbox path in `%LOCALAPPDATA%\Dropbox\info.json` (or `%APPDATA%\…`), or has a path component named `Dropbox`, `iCloudDrive` or `iCloud Drive`. This is a heuristic, used only to warn.
- **Switch vault** (a "Vault" menu or button in the library header, showing the current vault name): refused while a deck is playing. Otherwise it sends a new `Command::Eject(deck)` for both decks, drops the analysis pool (whose `Drop` already joins its workers), waits for any import to finish (switching is disabled during an import), opens the new vault and starts a fresh pool. `Eject` reuses the `Load` path's retire queue: it is refused while playing and does nothing if the retire queue is full (the app retries next frame), so the audio thread still never frees memory.
- **Broken link icon:** a new `assets/icons/link-broken.svg` in the style of the existing 16px stroke icons, drawn in `theme::DANGER` in the column where "missing" is shown today (library and set views), with a tooltip. The details panel's "File is missing" becomes the same icon plus "Not in the vault" and a **Locate file…** button, also in the row's context menu.
- **Removal dialog and notice** are reworded per the spec. The "file is still on your disk" notice is replaced.

### 11. Devtools

`--import-folder` and `--analyse` open the `last_vault` from settings, and fail with "open a vault in djkitty first" if there isn't one.

## Risks / Trade-offs

- [Imports are much slower and use as much disk again] → per-file progress with byte counts. Re-importing is fast (Decision 3's fast path). Cancelling an import is still out of scope, as it is today.
- [FAT32 USB sticks can't hold files over 4 GB, and exFAT/FAT have no symlinks or permissions] → an oversized file fails to copy and is counted as skipped. Nothing in the design relies on symlinks or hard links.
- [Long paths over 260 characters on Windows] → names are capped at 100 characters per component. Rust's std uses verbatim paths for long paths internally. A copy that still fails is skipped and logged.
- [The cloud-sync heuristic misses some sync tools] → it is a warning only, and the WAL database is no worse off than today's.
- [Hand-edited `library.db` paths] → Decision 6's guard. Imports never write a path that isn't produced by the naming function.
- [`canonicalize` fails on a missing file] → nothing to delete, so the result is a no-op with a log entry.
- [Case-insensitive name matching on case-sensitive filesystems] → it can only add an unnecessary ` (2)`, never overwrite, so it is safe.
- [Orphan Catalog files after a crash or a failed delete] → harmless, and they occupy their names so nothing is overwritten. Cleaning them up is a named follow-up.

## Migration Plan

None, because this change is breaking. The first launch after upgrading shows the welcome screen. The old `%APPDATA%\djkitty\data\library.db` and waveforms are left in place and never read or deleted. `settings.txt` keeps the audio device and column widths. The README describes vaults and drops the old "never edits, moves or deletes your audio files" wording in favour of "never changes your original files; it keeps its own copy in your vault".
