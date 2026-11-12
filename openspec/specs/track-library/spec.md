# track-library Specification

## Purpose
Lets users bring their music into djkitty by importing folders, keeps the resulting track collection between sessions, and lets them browse and search it.

## Requirements

### Requirement: Import a music folder
The system SHALL let the user choose a folder and SHALL add every supported audio file inside it, including subfolders, to the library. Supported formats in v0.1 are MP3, FLAC, WAV, AIFF and M4A/AAC.

#### Scenario: Folder with supported files
- **WHEN** the user imports a folder containing 3 MP3 files and 2 FLAC files
- **THEN** the library contains those 5 tracks

#### Scenario: Unsupported files are skipped
- **WHEN** the imported folder contains a `.txt` file and a `.jpg` file alongside audio files
- **THEN** only the audio files are added and the import completes without error

#### Scenario: Unreadable audio file
- **WHEN** a file has a supported extension but cannot be decoded
- **THEN** the file is not added, the rest of the import continues, and the user is told how many files were skipped

### Requirement: Re-importing does not duplicate tracks
The system SHALL identify tracks by file path, so importing a folder that is already in the library adds only new files.

#### Scenario: Re-import same folder
- **WHEN** the user imports a folder that was imported before and one new file has been added to it
- **THEN** exactly one new track is added and existing tracks are unchanged

### Requirement: Library persists between sessions
The system SHALL store the library locally so that tracks, their analysis results and sets are available after the app restarts.

#### Scenario: Restart
- **WHEN** the user imports tracks, closes djkitty and reopens it
- **THEN** the same tracks appear in the library without re-importing

### Requirement: Missing files are flagged
The system SHALL mark a track as missing when its file no longer exists at its stored path, and SHALL NOT allow a missing track to be loaded onto a deck.

#### Scenario: File deleted on disk
- **WHEN** a track's file is deleted outside djkitty and the library is shown
- **THEN** the track is shown as missing and cannot be loaded

### Requirement: Browse and search the library
The system SHALL list library tracks with title, artist, BPM, key, loudness (LUFS) and duration, SHALL let the user sort by any of these columns, and SHALL filter the list by a text search over title and artist.

#### Scenario: Search by artist
- **WHEN** the user types part of an artist's name into the search box
- **THEN** only tracks whose title or artist contains that text (case-insensitive) are listed

#### Scenario: Sort by BPM
- **WHEN** the user clicks the BPM column header
- **THEN** tracks are ordered by BPM, and tracks without a BPM are listed last

### Requirement: Track metadata from tags
The system SHALL read title and artist from the file's embedded tags and SHALL fall back to the file name as title when tags are absent.

#### Scenario: Untagged file
- **WHEN** a file named `my_mix_track.mp3` with no tags is imported
- **THEN** its title is shown as `my_mix_track` and its artist is empty
