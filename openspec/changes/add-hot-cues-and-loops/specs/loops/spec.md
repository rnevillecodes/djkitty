# Spec Delta

## Purpose

Lets the DJ repeat a section of a track a set number of beats long, from the controller pads or the screen. Loops start exactly where the DJ presses, so timing them stays a manual skill.

## ADDED Requirements

### Requirement: Beat loop lengths
The system SHALL offer eight beat loop lengths per deck: 1/4, 1/2, 1, 2, 4, 8, 16 and 32 beats, on pads 1 to 8 in that order. A beat's duration SHALL come from the tempo of the track's beatgrid, so pitch changes don't change how many beats a loop holds.

#### Scenario: Length from the grid tempo
- **WHEN** a track with a 128 BPM grid is playing at +4% pitch and the DJ starts a 4-beat loop
- **THEN** the loop holds exactly 4 beats of the track (1.875 s of track time), heard as 4 beats at the faster tempo

### Requirement: Start a beat loop
The system SHALL start a loop of the chosen length at the deck's current playhead position, exactly where the playhead is with no snapping to the beatgrid, whether the deck is playing or paused.

#### Scenario: Start while playing
- **WHEN** deck A is playing at 1:00.200 with a 120 BPM grid and the DJ presses the 2-beat loop pad
- **THEN** deck A loops from 1:00.200 to 1:01.200 until the loop is exited

#### Scenario: Not snapped to the grid
- **WHEN** the DJ starts a loop 40 ms after a beat marker
- **THEN** the loop starts 40 ms after that beat marker, not on it

#### Scenario: Start while paused
- **WHEN** deck B is paused and the DJ presses the 8-beat loop pad, then presses play
- **THEN** deck B plays from the playhead and repeats the 8-beat loop

### Requirement: Seamless looping
The system SHALL return to the loop start each time playback reaches the loop end, without a gap and without an audible click, and the waveform SHALL stay in sync with the audio across the jump.

#### Scenario: Repeating a loop
- **WHEN** a 1-beat loop plays for 30 seconds
- **THEN** the audio repeats evenly with no gaps, clicks or drift and the playhead never passes the loop end

### Requirement: Resize or exit a loop
The system SHALL exit the active loop and continue playing past its end when the DJ presses the active loop length again. Pressing a different length SHALL resize the loop to that length, keeping the same loop start.

#### Scenario: Exit by pressing the active length
- **WHEN** a 4-beat loop is active on deck A and the DJ presses the 4-beat pad again
- **THEN** the loop ends and deck A plays on through the rest of the track

#### Scenario: Halve the loop
- **WHEN** an 8-beat loop starting at 0:40 is active and the DJ presses the 4-beat pad
- **THEN** the loop becomes 4 beats long, still starting at 0:40

### Requirement: Leaving the loop region ends the loop
The system SHALL end the active loop whenever the playhead is moved to a position outside it, for example by jumping to a hot cue, using the main cue or seeking on the overview.

#### Scenario: Hot cue outside the loop
- **WHEN** a loop from 1:00 to 1:02 is active and the DJ presses a hot cue set at 2:30
- **THEN** the loop ends and playback continues from 2:30

### Requirement: Loops need a tempo
The system SHALL refuse to start a beat loop on a deck whose track has no BPM, and SHALL tell the DJ why.

#### Scenario: Track without a BPM
- **WHEN** the track on deck A has no detected or set BPM and the DJ presses a beat loop pad
- **THEN** no loop starts and a message says the track needs a BPM to loop

### Requirement: Loop at the end of the track
The system SHALL shorten a loop that would run past the end of the track so that it ends at the track's end.

#### Scenario: Loop near the end
- **WHEN** the DJ starts a 32-beat loop 5 seconds before the end of the track
- **THEN** the loop runs from that point to the end of the track and repeats

### Requirement: Loops on screen
The system SHALL show the eight loop lengths as on-screen pads per deck, with the active length lit, which start, resize and exit loops like the controller pads, so loops work with no controller connected.

#### Scenario: On-screen loop without a controller
- **WHEN** no controller is connected and the DJ clicks the on-screen 1-beat loop pad on a playing deck
- **THEN** a 1-beat loop starts at the playhead and the 1-beat pad is lit

### Requirement: Active loop on waveforms
The system SHALL shade the active loop's region on the deck's scrolling waveform and overview, with its start and end clearly marked.

#### Scenario: Loop region shown
- **WHEN** a 4-beat loop is active on deck B
- **THEN** deck B's scrolling waveform and overview show the shaded loop region between its start and end
