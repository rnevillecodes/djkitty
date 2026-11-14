# Spec Delta

## ADDED Requirements

### Requirement: Sets are mirrored as playlist files
The system SHALL keep one M3U8 playlist file per set in the vault's `Sets` folder, named after the set, listing the set's tracks in set order by paths relative to that file into the Catalog. The file SHALL be updated whenever the set is created, renamed, reordered, or gains or loses a track, and SHALL contain no audio.

#### Scenario: New set with tracks
- **WHEN** the user creates a set named "Friday warmup" and adds two tracks
- **THEN** `Sets\Friday warmup.m3u8` exists and lists both tracks in order, by paths into `..\Catalog`

#### Scenario: Reorder
- **WHEN** the user moves the 3rd track of a set to the top
- **THEN** the set's playlist file lists that track first

#### Scenario: Track removed from the library
- **WHEN** a track in 2 sets is removed from the library
- **THEN** neither set's playlist file lists it any more

#### Scenario: Opened in another player
- **WHEN** the user opens a set's playlist file in another music player, from the vault on a different computer
- **THEN** the player finds and plays every track in the set whose Catalog file exists

### Requirement: Set files follow the set's name and life
The system SHALL rename a set's playlist file when the set is renamed and delete it when the set is deleted, without changing any track or Catalog file. Characters Windows does not allow in file names SHALL be replaced, and when two sets would get the same file name the later one SHALL get a ` (2)`, ` (3)` suffix.

#### Scenario: Rename a set
- **WHEN** the user renames set "Friday warmup" to "Friday peak"
- **THEN** `Sets\Friday peak.m3u8` exists and `Sets\Friday warmup.m3u8` does not

#### Scenario: Delete a set
- **WHEN** the user deletes a set that contains 5 tracks
- **THEN** its playlist file is deleted, and all 5 tracks and their Catalog files remain

#### Scenario: Name not allowed on Windows
- **WHEN** the user creates a set named "Warmup 22:00"
- **THEN** the set is shown as "Warmup 22:00" and its playlist file name contains no `:`

### Requirement: The library is the source of truth for sets
The system SHALL treat its library as the record of each set's name, tracks and order, and SHALL rewrite every set's playlist file from it when the vault opens. Changes made to playlist files outside djkitty SHALL NOT change any set, and playlist files djkitty did not create SHALL be left alone.

#### Scenario: Playlist edited by hand
- **WHEN** the user deletes a line from a set's playlist file in a text editor and reopens the vault
- **THEN** the set still has all its tracks and the playlist file lists them again

#### Scenario: Foreign playlist file
- **WHEN** the user puts their own `Old crate.m3u8` in the `Sets` folder
- **THEN** no set is created from it and djkitty never changes or deletes it
