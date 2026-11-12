# Spec Delta

## Purpose

Gives each deck a single bipolar filter knob, like the CFX knob on Pioneer DJ controllers: low-pass to the left, high-pass to the right, and no effect at the centre. It is a manual mixing tool for transitions; nothing about it is tempo-synced or automatic.

## ADDED Requirements

### Requirement: Bipolar filter per deck
The system SHALL give each deck one filter control whose centre position means "off". Moving it left of centre SHALL apply a low-pass filter whose cutoff falls the further left it goes; moving it right of centre SHALL apply a high-pass filter whose cutoff rises the further right it goes. Each deck's filter SHALL be independent.

#### Scenario: Low-pass to the left
- **WHEN** deck A plays a full-range track and its filter is turned fully left
- **THEN** deck A's highs and mids are removed and mostly bass is heard

#### Scenario: High-pass to the right
- **WHEN** deck A plays a full-range track and its filter is turned fully right
- **THEN** deck A's bass is removed and mostly highs are heard

#### Scenario: Decks are independent
- **WHEN** deck A's filter is turned fully left and deck B's is centred
- **THEN** deck B's sound is unchanged

### Requirement: Centre is bypassed
The system SHALL leave a deck's sound unchanged while its filter is at or within a small dead zone around the centre, with no phase shift or level change.

#### Scenario: Centred filter
- **WHEN** a deck's filter is centred
- **THEN** that deck's output is identical to its output with no filter at all

#### Scenario: Slightly off centre
- **WHEN** a deck's filter is moved by less than the dead zone from the centre
- **THEN** that deck's sound is still unchanged

### Requirement: Cutoff range and character
The system SHALL sweep the low-pass cutoff from about 20 kHz just outside the dead zone down to about 100 Hz at the far left, and the high-pass cutoff from about 20 Hz just outside the dead zone up to about 8 kHz at the far right, spaced so that equal knob movements give equal musical steps (logarithmic in frequency). The filter SHALL roll off at 12 dB per octave with a gentle resonant peak near the cutoff, and SHALL NOT self-oscillate or raise the output level by more than a few dB.

#### Scenario: Gradual sweep
- **WHEN** a deck's filter is turned slowly from the centre to the far left
- **THEN** the sound darkens smoothly and evenly across the whole travel, rather than changing mostly at one end

#### Scenario: Far left
- **WHEN** a deck's filter is fully left
- **THEN** content well above about 100 Hz is strongly attenuated

### Requirement: Smooth filter changes
The system SHALL change the filter without audible clicks, zipper noise or dropouts, however fast the control is moved, including when it crosses the centre between low-pass and high-pass.

#### Scenario: Fast sweep across the centre
- **WHEN** a deck's filter is moved quickly from fully left to fully right while a track plays
- **THEN** the sweep is heard without clicks or gaps

### Requirement: Filter position in the signal path
The system SHALL apply each deck's filter after its EQ and before its channel fader, so that the master output, the headphone cue and the channel level meter all reflect the filtered sound.

#### Scenario: Headphone cue hears the filter
- **WHEN** deck B is cued in the headphones with its channel fader down and its filter fully right
- **THEN** the headphones play deck B high-passed

### Requirement: Filter on screen
The system SHALL show each deck's filter as a knob in that deck's mixer channel, labelled FILTER, which shows how far and in which direction it is turned from the centre. The label SHALL be highlighted while the filter is active (outside the dead zone). Hovering the knob SHALL show the filter type and cutoff frequency, or that it is off. Double-click or Ctrl+click SHALL return it to the centre. Showing the knob SHALL NOT change the size of the mixer or any other part of the layout.

#### Scenario: Tooltip shows the cutoff
- **WHEN** deck A's filter is turned partly left and the DJ hovers it
- **THEN** the tooltip names the low-pass filter and its current cutoff in Hz or kHz

#### Scenario: Reset to centre
- **WHEN** deck B's filter is turned fully right and the DJ double-clicks it
- **THEN** deck B's filter returns to the centre and its sound is unchanged

#### Scenario: Active label
- **WHEN** deck A's filter is moved out of the dead zone
- **THEN** its FILTER label is highlighted, and it returns to normal when the filter is centred

### Requirement: Filter is not saved
The system SHALL start each session with every filter centred and SHALL NOT store filter positions with tracks or settings. Loading a track SHALL NOT move the filter.

#### Scenario: New session
- **WHEN** djkitty starts with no controller connected
- **THEN** both decks' filters are centred
