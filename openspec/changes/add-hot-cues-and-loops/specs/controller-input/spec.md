# Spec Delta

## ADDED Requirements

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
The system SHALL map a deck's eight pads in Beat Loop mode to that deck's loop lengths, from 1/4 beat on pad 1 to 32 beats on pad 8, to start, resize and exit loops.

#### Scenario: Loop from the pads
- **WHEN** deck B is in Beat Loop mode and playing, and the DJ presses pad 5
- **THEN** a 4-beat loop starts at deck B's playhead

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

## MODIFIED Requirements

### Requirement: Unmapped controls are ignored
The system SHALL ignore MIDI messages from controls that have no action (such as FX buttons, BEAT SYNC without SHIFT, and pads in modes djkitty does not support) without errors or side effects.

#### Scenario: Pressing an FX button
- **WHEN** the user presses an FX button on the DDJ-FLX2
- **THEN** nothing changes in the app and no error is shown
