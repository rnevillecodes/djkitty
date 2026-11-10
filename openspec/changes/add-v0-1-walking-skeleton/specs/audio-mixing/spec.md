# Spec Delta

## Purpose

Combines the two decks into what the audience hears (master) and what the DJ hears in headphones (cue), with the faders needed to blend one track into another, delivered to the audio device with low latency.

## ADDED Requirements

### Requirement: Channel faders
The system SHALL scale each deck's level in the master mix by its channel fader, from silent at the bottom to full level at the top.

#### Scenario: Fader down
- **WHEN** deck A is playing and its channel fader is fully down
- **THEN** deck A is silent in the master output

### Requirement: Crossfader
The system SHALL blend decks A and B in the master mix with a crossfader: fully left plays only A, fully right plays only B, and the centre plays both at full level. The crossfader SHALL apply after the channel faders.

#### Scenario: Crossfader left
- **WHEN** both decks are playing with faders up and the crossfader is fully left
- **THEN** only deck A is heard in the master output

#### Scenario: Crossfader centre
- **WHEN** the crossfader is centred
- **THEN** both decks are heard in the master output at their channel-fader levels

### Requirement: Master level and clipping protection
The system SHALL provide a master level control and SHALL prevent the master output from hard-clipping digitally, by limiting it rather than wrapping or distorting harshly.

#### Scenario: Two loud tracks
- **WHEN** two loud tracks play with both channel faders and master at maximum and the crossfader centred
- **THEN** the master output stays within full scale without digital wrap-around

### Requirement: Headphone cue
The system SHALL let the user enable headphone cue per deck. The headphone output SHALL carry the cue-enabled decks independently of the channel faders and crossfader, and SHALL have its own level control.

#### Scenario: Pre-listen a track
- **WHEN** deck B's channel fader is down and headphone cue is enabled for deck B
- **THEN** deck B is heard in the headphones and not in the master output

#### Scenario: No cue selected
- **WHEN** headphone cue is disabled on both decks
- **THEN** the headphones carry the master mix

### Requirement: Output routing
The system SHALL route master and headphones to separate stereo pairs on the selected audio device when it has at least 4 output channels (DDJ-FLX2: master on 1–2, headphones on 3–4), and SHALL fall back to master-only on a stereo device.

#### Scenario: DDJ-FLX2 audio device
- **WHEN** the DDJ-FLX2's audio device is selected
- **THEN** master is heard on its master output and headphone cue on its headphone output

#### Scenario: Laptop speakers only
- **WHEN** a 2-channel device is selected
- **THEN** the master mix plays on it and the UI shows that headphone cue is unavailable

### Requirement: Low-latency, dropout-free output
The system SHALL deliver audio with an output latency of 20 ms or less on a supported device under typical settings, and SHALL NOT produce audible dropouts during normal use, including while the library is being analysed or the UI is busy.

#### Scenario: Responsive play button
- **WHEN** the user presses play on the controller
- **THEN** the sound starts within 20 ms of output latency plus input handling

#### Scenario: Sustained playback
- **WHEN** both decks play continuously for 30 minutes while the user browses and analysis runs
- **THEN** no dropouts or glitches occur

### Requirement: Audio device selection
The system SHALL list available output devices, SHALL automatically prefer the DDJ-FLX2 device when connected, SHALL remember the user's choice, and SHALL keep running if the device disconnects.

#### Scenario: Device unplugged
- **WHEN** the selected audio device is unplugged during playback
- **THEN** djkitty does not crash, shows that the device was lost, and lets the user pick another device or reconnect
