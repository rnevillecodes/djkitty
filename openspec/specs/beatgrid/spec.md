# beatgrid Specification

## Purpose
Shows where the beats and bars of each track fall, like the beat markers on a CDJ, and lets the DJ correct them, so beginners can learn to beatmatch and count phrases by eye and ear. It is a visual aid only: djkitty never matches tempos or aligns decks on the DJ's behalf.

## Requirements

### Requirement: Detected beatgrid
The system SHALL detect, during track analysis, a constant-tempo beatgrid for each track with a clear beat: its tempo and where its beats fall. The grid SHALL start at the beat where the music becomes audible, which is bar 1 beat 1, with no beats in any silence before it. Tracks without a clear beat SHALL have no grid.

#### Scenario: Grid lands on the beats
- **WHEN** a track with a steady beat finishes analysing
- **THEN** its grid's beats fall on the track's kicks and snares to within a few milliseconds

#### Scenario: Grid stays aligned through the track
- **WHEN** a five-minute track with a steady tempo is analysed
- **THEN** the grid is still on the beat at the end of the track

#### Scenario: Grid starts with the music
- **WHEN** a track has five seconds of silence before the music starts
- **THEN** its first beat marker, and bar 1 beat 1, are where the music starts, and there are no markers in the silence

#### Scenario: No beat
- **WHEN** a beatless ambient track is analysed
- **THEN** it has no grid and no beat markers are shown

### Requirement: Beat and bar markers on waveforms
The system SHALL draw the beatgrid on each deck's scrolling waveform: a subtle marker on every beat and a stronger marker, with the bar number when there is room, on every downbeat, with phrase starts (every 16 bars) highlighted. The overview SHALL mark every 16 bars.

#### Scenario: Markers scroll with the music
- **WHEN** a deck with a gridded track is playing
- **THEN** beat and bar markers move with the waveform and pass the playhead on the beat

#### Scenario: Phrases in the overview
- **WHEN** a gridded track is loaded
- **THEN** its overview shows a tick every 16 bars

### Requirement: Bar and beat position
The system SHALL show, for each deck with a gridded track, the current bar and beat (for example 33.2), with beat 1 of each bar highlighted.

#### Scenario: Counting bars
- **WHEN** a deck plays through the downbeat of bar 33
- **THEN** its position readout changes from 32.4 to 33.1

#### Scenario: Before the music
- **WHEN** a deck is positioned in the silence before a track's first beat
- **THEN** its position readout shows no bar or beat

### Requirement: Manual beatgrid adjustment
The system SHALL let the DJ correct a track's grid from its waveform: make the beat at the playhead beat 1 of a bar (snapping to the nearest transient), shift the whole grid earlier or later in millisecond steps or by dragging, halve or double the tempo, and set an exact tempo, keeping the bar near the playhead in place.

#### Scenario: Set bar 1 here
- **WHEN** the DJ pauses near a kick and chooses to set bar 1 there
- **THEN** that kick becomes beat 1 of a bar, the bar numbers follow from it, and the grid still starts where it did

#### Scenario: Wrong tempo octave
- **WHEN** a grid was detected at half the real tempo and the DJ doubles it
- **THEN** a marker falls on every real beat and the bar near the playhead stays where it was

#### Scenario: Fine alignment
- **WHEN** the grid sits slightly ahead of the beats and the DJ shifts it later
- **THEN** every marker moves by the chosen amount

### Requirement: Grid edits persist
The system SHALL save the DJ's grid edits in the library, keep them when the track is re-analysed, and offer a reset back to the detected grid. Grid edits SHALL NOT modify the audio file.

#### Scenario: Edit survives re-analysis
- **WHEN** the DJ adjusts a track's grid and the track is later re-analysed
- **THEN** the adjusted grid is kept

#### Scenario: Reset
- **WHEN** the DJ resets an adjusted grid
- **THEN** the detected grid is shown again

### Requirement: No automatic beat sync
The system SHALL NOT change a deck's tempo or position to match the other deck, and SHALL NOT snap actions to the grid, so that beatmatching stays a manual skill the DJ learns.

#### Scenario: Two gridded tracks
- **WHEN** two gridded tracks at different tempos play together
- **THEN** neither deck's tempo or position changes unless the DJ moves its pitch fader, jog wheel or transport controls
