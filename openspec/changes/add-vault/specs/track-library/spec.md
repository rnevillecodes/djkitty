# Spec Delta

## MODIFIED Requirements

### Requirement: Import a music folder
The system SHALL let the user choose a folder and SHALL add every supported audio file inside it, including subfolders, to the library by copying it into the open vault's Catalog. Supported formats in v0.1 are MP3, FLAC, WAV, AIFF and M4A/AAC. Every import SHALL be copied; there is no option to add a file without copying it.

#### Scenario: Folder with supported files
- **WHEN** the user imports a folder containing 3 MP3 files and 2 FLAC files
- **THEN** the library contains those 5 tracks, and each one has a copy in the vault's Catalog

#### Scenario: Unsupported files are skipped
- **WHEN** the imported folder contains a `.txt` file and a `.jpg` file alongside audio files
- **THEN** only the audio files are added and copied, and the import completes without error

#### Scenario: Unreadable audio file
- **WHEN** a file has a supported extension but cannot be decoded
- **THEN** the file is not added or copied, the rest of the import continues, and the user is told how many files were skipped

### Requirement: Re-importing does not duplicate tracks
The system SHALL identify tracks by the content of their audio file, so importing a file that is already in the vault, from its original location or from anywhere else, adds no track and copies nothing.

#### Scenario: Re-import same folder
- **WHEN** the user imports a folder that was imported before and one new file has been added to it
- **THEN** exactly one new track is added and copied, and existing tracks are unchanged

#### Scenario: Same file from two places
- **WHEN** the user imports `Downloads\Glue.mp3` and later imports a byte-identical copy at `USB\Promos\Glue.mp3`
- **THEN** the library has one track for it and the Catalog holds one copy

### Requirement: Library persists between sessions
The system SHALL store the library inside the open vault so that tracks, their analysis results, hot cues, beatgrids and sets are available after the app restarts, and on any computer the vault is opened on.

#### Scenario: Restart
- **WHEN** the user imports tracks, closes djkitty and reopens it
- **THEN** the same tracks appear in the library without re-importing

### Requirement: Missing files are flagged
The system SHALL mark a track with a broken link icon wherever it is listed, in the library and in sets, when its file is not in the vault's Catalog, and SHALL NOT allow such a track to be loaded onto a deck. The icon SHALL explain on hover that the file is not in the vault.

#### Scenario: File deleted on disk
- **WHEN** a track's Catalog file is deleted outside djkitty and the library is shown
- **THEN** the track shows the broken link icon and cannot be loaded

#### Scenario: Broken link in a set
- **WHEN** a set contains a track whose Catalog file is missing
- **THEN** the track keeps its position in the set, shows the broken link icon, and cannot be loaded from the set

## ADDED Requirements

### Requirement: Catalog layout
The system SHALL store each imported file at `Catalog\<artist>\<original file name>` in the vault, using the track's artist tag, or `Unknown Artist` when it has none. Characters Windows does not allow in names SHALL be replaced. When a different file already uses that name, ignoring letter case, the system SHALL add ` (2)`, ` (3)` and so on before the extension.

#### Scenario: Tagged file
- **WHEN** the user imports `02 Glue.mp3` tagged with artist "Bicep"
- **THEN** it is copied to `Catalog\Bicep\02 Glue.mp3`

#### Scenario: Untagged file
- **WHEN** the user imports `my_mix_track.mp3` with no artist tag
- **THEN** it is copied to `Catalog\Unknown Artist\my_mix_track.mp3`

#### Scenario: Different file with the same name
- **WHEN** `Catalog\Unknown Artist\01 Intro.mp3` exists and the user imports a different untagged file named `01 Intro.mp3`
- **THEN** the new file is copied to `Catalog\Unknown Artist\01 Intro (2).mp3` and the existing file is unchanged

#### Scenario: Name not allowed on Windows
- **WHEN** the user imports a file tagged with artist "AC/DC: Live"
- **THEN** it is copied into an artist folder whose name contains no `/` or `:`, and the track still shows the artist "AC/DC: Live"

### Requirement: Original files are never changed
The system SHALL NOT modify, move, rename or delete any file outside the vault, in any operation, including importing, removing a track and locating a file.

#### Scenario: Import then remove
- **WHEN** the user imports a folder, removes every imported track from the library, and then checks the original folder
- **THEN** every original file is still there, byte for byte

### Requirement: Vault copies are never modified
The system SHALL keep each Catalog file byte-identical to the file it was copied from, and SHALL NOT write tags, rename it or re-encode it. Edits the user makes to a track, such as hot cues and beatgrids, SHALL be stored in the library and not in the audio file.

#### Scenario: Edit a beatgrid
- **WHEN** the user sets hot cues and corrects the beatgrid of a track
- **THEN** the track's Catalog file is byte for byte the same as before

### Requirement: Imports are complete or absent
The system SHALL add a track to the library only after its Catalog file has been fully copied and confirmed to match the original's content. A file that changes or fails to copy during import SHALL be skipped and counted as skipped, without leaving a partial file in the Catalog.

#### Scenario: Disk full during import
- **WHEN** the vault's drive fills up partway through copying a file
- **THEN** that file is not added, no partial copy is left in the Catalog, and the user is told the import could not finish

### Requirement: Remove a track deletes its vault copy
The system SHALL let the user remove a track from the library after a confirmation that says the vault copy will be permanently deleted with its hot cues and beatgrid, says how many sets it is in, and says the original file is not affected. Removing SHALL delete the track from every set and permanently delete its Catalog file.

#### Scenario: Remove a track in two sets
- **WHEN** the user removes a track that is in 2 sets and confirms
- **THEN** the track is gone from the library and both sets, and its Catalog file no longer exists

#### Scenario: Cancel
- **WHEN** the user cancels the removal confirmation
- **THEN** the track, its Catalog file and its place in sets are unchanged

#### Scenario: Import again after removal
- **WHEN** the user removes a track and then imports its original file again
- **THEN** it is added and copied as a new track with no hot cues or edited beatgrid

### Requirement: Deletion is confined to the Catalog
The system SHALL delete an audio file only when its real location, after resolving any links and relative parts, is inside the open vault's Catalog folder. Otherwise it SHALL refuse to delete it, still remove the track from the library, and log the refusal.

#### Scenario: Library points outside the Catalog
- **WHEN** a library entry has been altered so that it points to a file outside the Catalog, and the user removes that track
- **THEN** the track is removed from the library and the outside file is not deleted

### Requirement: Locate a missing file
The system SHALL let the user repair a broken link by choosing a file anywhere on the computer. If the file's content matches the track's, the system SHALL copy it back into the Catalog at the track's location and clear the broken link, keeping its hot cues, beatgrid and sets. If the content does not match, it SHALL refuse and say so.

#### Scenario: Locate the original
- **WHEN** a track shows a broken link and the user chooses "Locate file…" and picks the original file it was imported from
- **THEN** the file is copied back into the Catalog, the broken link icon disappears, and the track loads with its hot cues

#### Scenario: Wrong file
- **WHEN** the user picks a different song while locating a missing file
- **THEN** nothing is copied, the broken link stays, and the user is told the file is not the same as the missing one
