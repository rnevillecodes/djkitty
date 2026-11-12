# Spec Delta

## Purpose

Gives each deck eight hot cues: saved points in a track the DJ can set, jump to, preview and clear, from the controller pads or the screen. They are kept with the track for future sessions.

## ADDED Requirements

### Requirement: Eight hot cue slots per deck
The system SHALL give each deck eight hot cue slots, numbered 1 to 8, each either empty or holding a position in the loaded track.

#### Scenario: Fresh track
- **WHEN** a track that has never had hot cues is loaded onto deck A
- **THEN** all eight of deck A's hot cue slots are empty

### Requirement: Set a hot cue
The system SHALL set an empty hot cue slot to the deck's current playhead position when it is pressed, exactly where the playhead is with no snapping to the beatgrid, whether the deck is playing or paused, without changing playback.

#### Scenario: Set while playing
- **WHEN** deck A is playing at 1:02.350 and the DJ presses empty hot cue 3
- **THEN** hot cue 3 is set at 1:02.350 and deck A keeps playing without interruption

#### Scenario: Set while paused
- **WHEN** deck B is paused at 0:31.000 and the DJ presses empty hot cue 1
- **THEN** hot cue 1 is set at 0:31.000 and deck B stays paused

#### Scenario: No track loaded
- **WHEN** no track is loaded on deck A and the DJ presses a hot cue
- **THEN** nothing happens

### Requirement: Jump to a hot cue while playing
The system SHALL move a playing deck's playhead to a set hot cue as soon as the hot cue is pressed, and the deck SHALL keep playing from there.

#### Scenario: Jump while playing
- **WHEN** deck A is playing at 2:10 and the DJ presses hot cue 2, set at 0:45
- **THEN** deck A continues playing from 0:45

### Requirement: Hot cue preview while paused
The system SHALL, when a set hot cue is pressed on a paused deck, move the playhead to the hot cue and play while the pad is held. On release it SHALL return to the hot cue and pause, unless play was pressed during the hold, in which case playback continues.

#### Scenario: Preview and release
- **WHEN** deck A is paused at 1:30 and the DJ holds hot cue 4, set at 0:20, for two seconds and releases it
- **THEN** deck A plays from 0:20 while held, then returns to 0:20 and pauses

#### Scenario: Keep playing from a preview
- **WHEN** the DJ holds hot cue 4 on a paused deck, presses play, then releases the hot cue
- **THEN** the deck keeps playing

### Requirement: Clear a hot cue
The system SHALL clear a set hot cue when the DJ presses it with SHIFT held on the controller, or chooses to clear it on screen, without changing playback.

#### Scenario: SHIFT clears
- **WHEN** hot cue 3 on deck A is set and the DJ holds SHIFT and presses pad 3 in Hot Cue mode
- **THEN** hot cue 3 becomes empty and deck A's playback is unchanged

### Requirement: Hot cues are kept with the track
The system SHALL save each track's hot cues in the library when they are set or cleared, and SHALL restore them whenever the track is loaded onto either deck, including after the app restarts.

#### Scenario: Restored after restart
- **WHEN** the DJ sets hot cues 1 and 5 on a track, quits djkitty, restarts it and loads the same track onto deck B
- **THEN** deck B shows hot cues 1 and 5 at the same positions and pressing them jumps there

### Requirement: Hot cues on screen
The system SHALL show each deck's hot cues as eight on-screen pads, with set slots lit and labelled with their number, which set, jump to, preview and clear hot cues like the controller pads, so hot cues work with no controller connected.

#### Scenario: On-screen pad without a controller
- **WHEN** no controller is connected and the DJ clicks empty on-screen hot cue 2 on deck A, then clicks it again later
- **THEN** the first click sets hot cue 2 at the playhead and the second jumps to it

### Requirement: Hot cue markers on waveforms
The system SHALL draw every set hot cue as a numbered marker at its position on the deck's scrolling waveform and overview, distinct from the main cue marker and the beatgrid markers.

#### Scenario: Marker appears
- **WHEN** the DJ sets hot cue 6 on deck A
- **THEN** a marker labelled 6 appears at that position on deck A's scrolling waveform and overview
