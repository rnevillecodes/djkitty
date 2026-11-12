# controller-input Specification

## Purpose
Lets DJs perform with physical hardware by detecting a supported controller, turning its MIDI messages into deck, mixer and library actions, and lighting its LEDs to match app state. The DDJ-FLX2 is the first supported controller.

## Requirements

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
- **THEN** the user can load, play, cue, adjust pitch, move faders, adjust EQ and toggle headphone cue on screen

### Requirement: DDJ-FLX2 deck controls
For each deck, the system SHALL map the controller's play/pause, cue, pitch fader and jog wheel to the matching deck actions in deck-playback.

#### Scenario: Play button
- **WHEN** the user presses the play/pause button on the left deck of the DDJ-FLX2
- **THEN** deck A toggles between playing and paused

#### Scenario: Pitch fader
- **WHEN** the user moves the right deck's pitch fader
- **THEN** deck B's tempo changes to match the fader position

### Requirement: DDJ-FLX2 mixer controls
The system SHALL map the controller's channel faders, EQ knobs (high, mid and low), crossfader and headphone cue buttons to the matching mixer actions. Controls that are analogue on the hardware (such as the master and headphone volume knobs on the DDJ-FLX2) are outside the software's control.

#### Scenario: Crossfader
- **WHEN** the user moves the crossfader fully right on the controller
- **THEN** the master mix plays only deck B

#### Scenario: Headphone cue button
- **WHEN** the user presses deck A's headphone cue button
- **THEN** headphone cue is toggled for deck A

#### Scenario: EQ knob
- **WHEN** the user turns the left channel's low EQ knob fully down on the controller
- **THEN** deck A's low band is cut

### Requirement: DDJ-FLX2 library controls
The system SHALL map the controller's browse knob to moving the selection in the visible track list (library or set), and its load buttons to loading the selected track onto the matching deck.

#### Scenario: Browse and load
- **WHEN** the user turns the browse knob two steps clockwise and presses the left load button
- **THEN** the selection moves down two tracks and that track is loaded onto deck A

### Requirement: Soft takeover for faders
The system SHALL ignore a controller fader or EQ knob whose physical position differs from the app's value, such as after the value was changed on screen, until the physical control passes through the app's value, to avoid sudden jumps.

On connect, before any value is changed on screen, the system SHALL ask the controller for its current positions and SHALL apply each control's first reported position directly, so the app matches the hardware.

#### Scenario: Value changed on screen
- **WHEN** deck A's channel fader was lowered on screen to 20% while the hardware fader sits at 100%, and the user then moves the hardware fader from 100% to 90%
- **THEN** the deck A level stays at 20% until the hardware fader reaches 20%, after which it follows the hardware

#### Scenario: Fader down when the controller connects
- **WHEN** the controller connects with its deck A channel fader all the way down
- **THEN** deck A's channel fader in the app moves to 0% without the hardware fader being touched

### Requirement: LED feedback
The system SHALL light the controller's play and cue LEDs to reflect each deck's state (play lit while playing; cue lit at the cue point or blinking when paused elsewhere) and headphone cue LEDs to reflect cue selection, and SHALL resync all LEDs on connect.

#### Scenario: Play LED
- **WHEN** deck B starts playing from the on-screen play button
- **THEN** the right deck's play LED on the controller lights up

#### Scenario: Resync on connect
- **WHEN** the controller is reconnected while deck A is playing
- **THEN** deck A's play LED is lit immediately after reconnect

### Requirement: Unmapped controls are ignored
The system SHALL ignore MIDI messages from controls that have no action (such as FX buttons, BEAT SYNC without SHIFT, and pads in modes djkitty does not support) without errors or side effects.

#### Scenario: Pressing a pad
- **WHEN** the user presses a performance pad while its deck is in a pad mode djkitty does not support (such as Pad FX or Sampler)
- **THEN** nothing changes in the app and no error is shown

#### Scenario: Pressing an FX button
- **WHEN** the user presses an FX button on the DDJ-FLX2
- **THEN** nothing changes in the app and no error is shown

### Requirement: DDJ-FLX2 pad mode selection
The system SHALL let the DJ choose each deck's pad mode on the DDJ-FLX2 by holding SHIFT and pressing that deck's BEAT SYNC, then pressing pad 1 for Hot Cue or pad 3 for Beat Loop. Each deck SHALL start in Hot Cue mode and SHALL keep its own mode.

#### Scenario: Switch deck A to Beat Loop
- **WHEN** deck A is in Hot Cue mode and the DJ holds SHIFT, presses deck A's BEAT SYNC, then presses deck A's pad 3
- **THEN** deck A's pads switch to Beat Loop mode, and deck B's pads stay in their own mode

#### Scenario: Unsupported mode chosen
- **WHEN** the DJ enters mode selection on deck B and presses pad 2 (Pad FX) or pad 4 (Sampler)
- **THEN** deck B's pad mode does not change and no error is shown

#### Scenario: Leaving selection without choosing
- **WHEN** the DJ enters mode selection on deck A and then presses SHIFT + BEAT SYNC again
- **THEN** mode selection ends and deck A keeps its previous mode

### Requirement: BEAT SYNC never syncs
The system SHALL NOT change any deck's tempo or position when BEAT SYNC is pressed, with or without SHIFT. Its only action SHALL be entering pad mode selection with SHIFT.

#### Scenario: BEAT SYNC alone
- **WHEN** two decks play at different tempos and the DJ presses deck B's BEAT SYNC without SHIFT
- **THEN** neither deck's tempo or position changes

### Requirement: DDJ-FLX2 pads in Hot Cue mode
The system SHALL map a deck's eight pads in Hot Cue mode to that deck's hot cues 1 to 8: pressing a pad sets, jumps to or previews the hot cue, releasing it ends a preview, and SHIFT + pad clears it.

#### Scenario: Set and jump from the pads
- **WHEN** deck A is in Hot Cue mode and playing, and the DJ presses pad 5 twice, a few seconds apart
- **THEN** the first press sets hot cue 5 and the second jumps back to it

### Requirement: DDJ-FLX2 pads in Beat Loop mode
The system SHALL map a deck's eight pads in Beat Loop mode to that deck's loop lengths, from 1/4 beat on pad 1 to 32 beats on pad 8, to start, resize and exit loops. SHIFT + any pad in Beat Loop mode SHALL exit the active loop.

#### Scenario: Loop from the pads
- **WHEN** deck B is in Beat Loop mode and playing, and the DJ presses pad 5
- **THEN** a 4-beat loop starts at deck B's playhead

#### Scenario: SHIFT + pad exits the loop
- **WHEN** a loop is active on deck B in Beat Loop mode and the DJ holds SHIFT and presses any pad
- **THEN** the loop ends and no new loop starts

### Requirement: On-screen pads follow the controller mode
The system SHALL switch a deck's on-screen pad row between Hot Cue and Loop to match that deck's pad mode on the controller, and changing the on-screen switch SHALL also change the controller's mode for that deck.

#### Scenario: Mode mirrored on screen
- **WHEN** the DJ switches deck A's pads to Beat Loop mode on the DDJ-FLX2
- **THEN** deck A's on-screen pad row shows the loop lengths

### Requirement: Pad LED feedback
The system SHALL light the DDJ-FLX2 pads to match each deck's state. In Hot Cue mode, pads with a set hot cue are lit. In Beat Loop mode, the pad for the active loop length is lit. During mode selection, the pads for the available modes are lit. Pad LEDs SHALL resync on connect.

#### Scenario: Hot cue pads light
- **WHEN** deck A is in Hot Cue mode and the loaded track has hot cues 1 and 4 set
- **THEN** pads 1 and 4 on deck A are lit and the other pads are dark

#### Scenario: Reconnect
- **WHEN** the controller is reconnected while deck B has an active 2-beat loop in Beat Loop mode
- **THEN** deck B's pad 4 is lit immediately after reconnect

### Requirement: DDJ-FLX2 CFX knobs drive the filters
The system SHALL map each channel's CFX knob on the DDJ-FLX2 to that deck's filter across its full travel, with the hardware centre as the filter's centre. The knobs SHALL use the same soft takeover and initial position report as the EQ knobs. The CFX (Smart CFX) button SHALL have no action.

#### Scenario: Turn the CFX knob
- **WHEN** the DJ turns the left channel's CFX knob fully left on the controller
- **THEN** deck A's filter moves fully to low-pass

#### Scenario: Takeover after an on-screen change
- **WHEN** deck B's filter was reset to the centre on screen while the hardware CFX knob sits fully right, and the DJ then turns the knob left
- **THEN** deck B's filter stays centred until the knob passes the centre, then follows the knob

#### Scenario: CFX button
- **WHEN** the DJ presses the CFX button
- **THEN** nothing happens
