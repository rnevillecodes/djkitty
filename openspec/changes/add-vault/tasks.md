# Tasks

## 1. Vault folder and schema

- [x] 1.1 Add `blake3` to the workspace and `djk-library` dependencies. Verify `cargo build --workspace` succeeds
- [x] 1.2 Add the `vault` module (design Decision 1): `Vault::create(parent, name)` (rejecting empty, disallowed or changed-by-cleaning names and an existing target), `Vault::open(root)` (requires `.djkitty\vault.toml`, refuses a newer `format`), and the layout paths. Verify with unit tests for each `vault` spec scenario under "Create a vault" and "Open an existing vault", including that opening a non-vault folder creates nothing in it
- [x] 1.3 Add `sanitize_component` and unique-name selection (design Decision 5). Verify with unit tests for disallowed characters, trailing dots and spaces, reserved names (`CON`, `nul.mp3`, `COM1`), the 100-character cap keeping the extension, the `Unknown Artist` fallback, and case-insensitive ` (2)`/` (3)` suffixing against both existing rows and existing files
- [x] 1.4 Replace the migration list with the single v1 schema (design Decision 2: `hash`, Catalog-relative `path COLLATE NOCASE`, `source_*`, `sets.file_name`, no dead grid columns) and replace `Library::open_in` with `Library::open(&Vault)`, which clears `.djkitty\incoming` and resolves stored paths to absolute ones in `TrackRow`, `loadable_path` and `tracks_needing_analysis`. Verify that `fresh_db_migrates_and_reopen_keeps_data` passes against a vault, that the old migration tests are deleted, and that a leftover `.part` file is gone after reopening (`vault` spec "Power cut during import")

## 2. Import into the Catalog

- [x] 2.1 Rewrite `import_paths` (design Decision 3): fast path on source path, size and mtime; hash; copy to `.part` while re-hashing; `sync_all`; name; rename into `Catalog\<artist>\`; insert and commit per file; delete the file if the insert fails; skip a file whose content changed mid-copy; stop the import on errors that aren't per-file. Verify with library tests for every MODIFIED `track-library` import scenario, "Same file from two places", every "Catalog layout" scenario, and that copies are byte-identical to the originals
- [x] 2.2 Test the failure paths: a source that changes between hash and copy (simulated through an injectable reader or hook) is skipped and leaves no `.part` or Catalog file, and an insert failure leaves no Catalog file. Verify the tests pass and cover `track-library` "Imports are complete or absent"
- [x] 2.3 Add a test that imports a folder, then snapshots the originals and checks they are byte for byte unchanged after import, re-import and removal of every track (`track-library` "Original files are never changed"). Verify it passes
- [x] 2.4 Add a test that a vault copied to a new location (different root path) opens with every track loadable and none missing (`vault` spec "Vaults are portable"). Verify it passes

## 3. Removal, broken links and Locate file

- [x] 3.1 Implement `delete_catalog_file` with the canonical-path guard and empty artist folder cleanup, and make `remove_track` delete the row, rewrite affected set files, then delete the file (design Decision 6). Verify with library tests for "Remove a track in two sets", "Import again after removal", and "Library points outside the Catalog" with an absolute path, a `..\..\` path and (where creatable) a junction or symlink, each leaving the outside file intact
- [x] 3.2 Make `refresh_missing` check Catalog paths under the vault root. Verify `deleted_file_is_flagged_missing_and_cannot_load` passes against a vault, plus a set-view case (track keeps its position and can't be loaded)
- [x] 3.3 Implement `relink(id, picked)` (design Decision 8): hash check, copy back through `incoming` to the stored path, the existing-file rules, and `NotSameFile`. Verify with library tests for "Locate the original" (hot cues and set entries still present afterwards) and "Wrong file" (nothing copied, still missing)
- [x] 3.4 Add a test that setting hot cues and editing the beatgrid leave the Catalog file byte-identical (`track-library` "Vault copies are never modified"). Verify it passes

## 4. Set playlist files

- [x] 4.1 Write a set's `.m3u8` after create, rename, add, remove, reorder and track removal, using a temp file and rename, with `sets.file_name` ownership, ` (2)` clashes and cleaned names (design Decision 7). Verify with library tests for every scenario in the `set-management` "Sets are mirrored as playlist files" and "Set files follow the set's name and life" requirements, checking file contents and that the relative paths resolve to existing Catalog files
- [x] 4.2 Rewrite all set files on `Library::open` and never touch unrecorded files. Verify with tests for "Playlist edited by hand" and "Foreign playlist file"
- [x] 4.3 Make a failure to write a set file get logged and returned as a warning, not an error. Verify with a test that makes `Sets\` read-only (or replaces it with a file) and checks the set change is still committed

## 5. Engine: eject

- [x] 5.1 Add `Command::Eject(DeckId)`: refused while playing, retires the buffer through the existing retire queue, does nothing when the queue is full, and leaves the deck empty with hot cues and loop cleared. Verify with deck unit tests, the `Command` size test and `no_alloc`

## 6. App: vaults

- [ ] 6.1 Extend `Settings` with `last_vault` and `recent_vaults` (five, most recent first, deduplicated). Verify with the settings round-trip test extended to the new keys
- [ ] 6.2 Add `DjApp::vault`. On startup open `last_vault`, or show the welcome screen with the reason. Block library, import, drag-and-drop and loading while no vault is open. Pass the vault root to the import thread, analysis pool and loader in place of `data_dir`. Verify by running the app with a fresh `DJKITTY_DATA_DIR`: the welcome screen shows, and dropping files does nothing (`vault` spec "First launch")
- [ ] 6.3 Build the create dialog (name, "Choose location…", full-path preview, inline errors) and the open flow (folder picker, "not a djkitty vault" message), plus recent vaults on the welcome screen. Verify by creating a vault on another drive or folder, restarting into it ("Restart"), and opening a plain folder ("Not a vault")
- [ ] 6.4 Add the cloud-sync check (design Decision 10) with "Continue anyway" / "Choose another folder". Verify with unit tests for the path heuristic (OneDrive env var, Dropbox `info.json`, `iCloudDrive` component), and manually by creating a vault under `%OneDrive%`
- [ ] 6.5 Add the vault switcher in the library header (current vault name; switch / create), disabled during an import and refused while a deck plays. Switching ejects both decks, restarts the analysis pool and opens the new vault. Verify "Switch vault" and "Switch while a deck is playing" in the app
- [ ] 6.6 Show the unavailable-vault message when `last_vault` can't be opened. Verify by creating a vault on a USB stick, unplugging it and starting the app ("USB stick not plugged in"), and checking nothing new was created
- [ ] 6.7 Point `--import-folder` and `--analyse` at `last_vault`, with the "open a vault in djkitty first" error. Verify both commands against a vault, and with a fresh `DJKITTY_DATA_DIR`

## 7. App: broken links, Locate file and removal

- [ ] 7.1 Add `assets/icons/link-broken.svg` (16px, matching the stroke style of `file.svg`) and draw it in `theme::DANGER` in the library and set views in place of the "missing" label, with the tooltip. Verify by deleting a Catalog file in Explorer and screenshotting both views at the minimum window size
- [ ] 7.2 Add "Locate file…" to the row context menu and the details panel (picker opens at the source folder when it exists), with success and "not the same file" notices. Verify "Locate the original" and "Wrong file" in the app, and that the track then loads with its hot cues
- [ ] 7.3 Reword the removal dialog (permanent vault delete, hot cues and beatgrid, set count, original not affected) and its notices, including the "could not delete the file" case. Verify "Remove a track in two sets" and "Cancel" in the app and check the Catalog folder in Explorer
- [ ] 7.4 Update the import notice to report bytes copied as well as files added, already in the vault, and skipped. Verify by importing a folder twice: the second import reports everything already in the vault and finishes quickly

## 8. Documentation

- [ ] 8.1 Update `README.md`: vaults (create, open, switch, layout, portability, cloud warning), copy on import, removal deleting the vault copy, broken links and Locate file, set playlist files, and the reworded promise about original files. Verify the README describes only built behaviour and no longer says djkitty never deletes audio files

## 9. End-to-end check

- [ ] 9.1 Create a vault, import a real folder (including a file with a reserved or invalid name, a duplicate copy and a same-named different file), build two sets, set hot cues and edit a grid, then copy the vault to a USB stick and open it on the stick (and on another computer if available). Verify every track loads with its cues and grid, the set files open in VLC, and the original folder is unchanged
- [ ] 9.2 Run `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` and `cargo test --workspace`. Verify all pass

## Workflow follow-up

- Archive this change after review, so the `vault` spec is created and the `track-library` and `set-management` deltas are applied.
