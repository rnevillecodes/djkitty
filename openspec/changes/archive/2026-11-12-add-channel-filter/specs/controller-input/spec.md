# Spec Delta

## ADDED Requirements

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
