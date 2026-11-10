# Spec Delta

## Purpose

Lets DJs perform with physical hardware by detecting a supported controller, turning its MIDI messages into deck, mixer and library actions, and lighting its LEDs to match app state. The DDJ-FLX2 is the first supported controller.

## ADDED Requirements

### Requirement: Controller detection
The system SHALL detect a connected DDJ-FLX2 over USB MIDI at startup and when it is plugged in while running, without manual configuration, and SHALL show the connection status in the UI.

#### Scenario: Plugged in at startup
- **WHEN** djkitty starts with a DDJ-FLX2 connected by USB
- **THEN** the UI shows the controller as connected and its controls work

#### Scenario: Hot-plug
- **WHEN** a DDJ-FLX2 is plugged in while djkitty is running
- **THEN** it is detected within 3 seconds and becomes usable

#### Scenario: Unplugged
- **WHEN** the controller is unplugged while djkitty is running
- **THEN** the UI shows it as disconnected, playback continues, and nothing crashes

### Requirement: Works without a controller
The system SHALL offer every v0.1 deck and mixer action through the on-screen UI, so the app is fully usable with mouse and keyboard alone.

#### Scenario: No controller
- **WHEN** no controller is connected
- **THEN** the user can load, play, cue, adjust pitch, move faders and toggle headphone cue on screen

### Requirement: DDJ-FLX2 deck controls
For each deck, the system SHALL map the controller's play/pause, cue, pitch fader and jog wheel to the matching deck actions in deck-playback.

#### Scenario: Play button
- **WHEN** the user presses the play/pause button on the left deck of the DDJ-FLX2
- **THEN** deck A toggles between playing and paused

#### Scenario: Pitch fader
- **WHEN** the user moves the right deck's pitch fader
- **THEN** deck B's tempo changes to match the fader position

### Requirement: DDJ-FLX2 mixer controls
The system SHALL map the controller's channel faders, crossfader and headphone cue buttons to the matching mixer actions. Controls that are analogue on the hardware (such as the master and headphone volume knobs on the DDJ-FLX2) are outside the software's control.

#### Scenario: Crossfader
- **WHEN** the user moves the crossfader fully right on the controller
- **THEN** the master mix plays only deck B

#### Scenario: Headphone cue button
- **WHEN** the user presses deck A's headphone cue button
- **THEN** headphone cue is toggled for deck A

### Requirement: DDJ-FLX2 library controls
The system SHALL map the controller's browse knob to moving the selection in the visible track list (library or set), and its load buttons to loading the selected track onto the matching deck.

#### Scenario: Browse and load
- **WHEN** the user turns the browse knob two steps clockwise and presses the left load button
- **THEN** the selection moves down two tracks and that track is loaded onto deck A

### Requirement: Soft takeover for faders
The system SHALL ignore a controller fader whose physical position differs from the app's value, such as after the value was changed on screen, until the physical fader passes through the app's value, to avoid sudden jumps.

#### Scenario: Value changed on screen
- **WHEN** deck A's channel fader was lowered on screen to 20% while the hardware fader sits at 100%, and the user then moves the hardware fader from 100% to 90%
- **THEN** the deck A level stays at 20% until the hardware fader reaches 20%, after which it follows the hardware

### Requirement: LED feedback
The system SHALL light the controller's play and cue LEDs to reflect each deck's state (play lit while playing; cue lit at the cue point or blinking when paused elsewhere) and headphone cue LEDs to reflect cue selection, and SHALL resync all LEDs on connect.

#### Scenario: Play LED
- **WHEN** deck B starts playing from the on-screen play button
- **THEN** the right deck's play LED on the controller lights up

#### Scenario: Resync on connect
- **WHEN** the controller is reconnected while deck A is playing
- **THEN** deck A's play LED is lit immediately after reconnect

### Requirement: Unmapped controls are ignored
The system SHALL ignore MIDI messages from controls that have no v0.1 action (such as pads, FX and sync buttons) without errors or side effects.

#### Scenario: Pressing a pad
- **WHEN** the user presses a performance pad on the DDJ-FLX2
- **THEN** nothing changes in the app and no error is shown
