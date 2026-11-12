# Spec Delta

## Purpose

Provides two independent decks that load and play tracks with the core transport and tempo controls a beginner needs to beatmatch by ear.

## ADDED Requirements

### Requirement: Two independent decks
The system SHALL provide two decks, A and B, each of which holds at most one loaded track and is controlled independently.

#### Scenario: Independent transport
- **WHEN** deck A is playing and the user presses play on deck B
- **THEN** both decks play at the same time and pausing one does not affect the other

### Requirement: Load a track
The system SHALL load a library track onto a deck, positioned at the start with the cue point at the start, and SHALL show its title, artist, BPM and key on that deck. Loading onto a playing deck SHALL be refused.

#### Scenario: Load onto a stopped deck
- **WHEN** the user loads a track onto stopped deck A
- **THEN** deck A shows the track's details and its playhead is at 0:00

#### Scenario: Load onto a playing deck
- **WHEN** the user tries to load a track onto deck A while deck A is playing
- **THEN** the load is refused, deck A keeps playing, and the user is told the deck is playing

### Requirement: Play and pause
The system SHALL start playback from the current position on play and SHALL hold the current position on pause.

#### Scenario: Pause and resume
- **WHEN** the user pauses a deck at 1:23 and presses play again
- **THEN** playback resumes from 1:23

#### Scenario: End of track
- **WHEN** a playing deck reaches the end of its track
- **THEN** the deck stops at the end of the track

### Requirement: Cue
The system SHALL support a cue point per deck using standard CDJ-style behaviour: pressing cue on a paused deck sets the cue point at the current position; pressing cue on a playing deck returns to the cue point and pauses; holding cue on a paused deck at the cue point plays until released.

#### Scenario: Set a cue point
- **WHEN** the deck is paused at 0:32 and the user presses cue
- **THEN** the cue point is set to 0:32

#### Scenario: Return to cue
- **WHEN** the deck is playing at 1:10 with the cue point at 0:32 and the user presses cue
- **THEN** the deck jumps to 0:32 and pauses

#### Scenario: Cue preview
- **WHEN** the deck is paused at its cue point and the user holds cue
- **THEN** the deck plays while held and returns to the cue point, paused, when released

### Requirement: Pitch fader tempo control
The system SHALL let each deck's playback speed be adjusted within ±8% (default) by a pitch fader, SHALL change pitch together with tempo (no key-lock in v0.1), and SHALL show the resulting BPM and percentage.

#### Scenario: Speed up
- **WHEN** a 125.0 BPM track is playing and its pitch fader is set to +2.4%
- **THEN** the deck plays 2.4% faster and shows 128.0 BPM and +2.4%

#### Scenario: Reset
- **WHEN** the pitch fader is at its centre position
- **THEN** the deck plays at the track's original speed and shows 0.0%

### Requirement: Jog nudge
The system SHALL temporarily speed up or slow down a playing deck while its jog wheel is turned, proportional to how fast it is turned, returning to the pitch-fader speed when it stops. On a paused deck turning the jog SHALL move the playhead.

#### Scenario: Nudge forward
- **WHEN** a playing deck's jog wheel is turned clockwise
- **THEN** the deck briefly plays faster and returns to its set tempo once the jog stops

#### Scenario: Seek while paused
- **WHEN** a paused deck's jog wheel is turned anticlockwise
- **THEN** the playhead moves backwards in the track

### Requirement: Position reporting
The system SHALL show each deck's elapsed and remaining time, updated at least 30 times per second while playing.

#### Scenario: Remaining time
- **WHEN** a 6:00 track is playing at 1:30
- **THEN** the deck shows 1:30 elapsed and -4:30 remaining
