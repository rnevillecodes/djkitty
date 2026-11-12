# waveform-display Specification

## Purpose
Shows DJs what their music looks like as it plays: a smoothly scrolling close-up waveform and a full-track overview per deck, so they can see where they are and what is coming next.

## Requirements

### Requirement: Scrolling deck waveform
The system SHALL show, for each loaded deck, a zoomed-in waveform centred on a fixed playhead marker that scrolls smoothly as the track plays, covering roughly 8 seconds of audio by default.

#### Scenario: Playing track scrolls
- **WHEN** a deck is playing
- **THEN** its waveform moves past the fixed playhead in sync with the audio

#### Scenario: Paused track holds
- **WHEN** a deck is paused
- **THEN** its waveform stops moving and the playhead marks the paused position

### Requirement: Waveform stays in sync with audio
The system SHALL keep the waveform position in step with what is heard, adjusted for output latency, so the visual offset is no more than one display frame, including when pitch or jog changes speed.

#### Scenario: Pitch change
- **WHEN** a deck's pitch fader is raised to +6%
- **THEN** its waveform scrolls 6% faster and stays aligned with the audio

### Requirement: Smooth rendering
The system SHALL render both deck waveforms at the display refresh rate, and at least 60 frames per second on a typical laptop, while both decks play.

#### Scenario: Both decks playing
- **WHEN** both decks are playing and the user is browsing the library
- **THEN** the waveforms scroll without visible stutter

### Requirement: Frequency colouring
The system SHALL colour waveforms by frequency band (low, mid, high) so kicks, basslines and hi-hats can be told apart, using a palette readable on the app's theme.

#### Scenario: Kick drum
- **WHEN** a section with a prominent kick drum and little else is shown
- **THEN** that part of the waveform is drawn in the low-band colour

### Requirement: Track overview with position
The system SHALL show a full-track overview waveform for each loaded deck with a marker for the current position and the cue point, and SHALL seek to a position when the user clicks the overview of a paused deck.

#### Scenario: Seeking from the overview
- **WHEN** the user clicks the middle of a paused deck's overview
- **THEN** the deck's position moves to the middle of the track

#### Scenario: Cue marker
- **WHEN** a cue point is set at 0:32
- **THEN** a cue marker is drawn on the overview at 0:32

### Requirement: Waveforms in library and sets
The system SHALL show a small overview waveform for each track in its details view so tracks can be judged before loading.

#### Scenario: Track details
- **WHEN** the user opens a track's details
- **THEN** its overview waveform is displayed without loading it on a deck
