# visual-style Specification

## Purpose
Defines how djkitty looks: a calm, professional "studio satin" style with neutral dark surfaces lit from above, stepped clearly enough that panels and controls stand out. Each deck carries its own colour as identity, across its panel, lane, toggles and mixer channel; areas shared by both decks use a single accent colour for selection and focus; and state colours (play, cue, hot cue, loop, meters, warning, danger) keep their meaning, so the controls read like real DJ equipment.

## Requirements

### Requirement: Neutral near-black palette
The system SHALL paint its surfaces in near-black neutral greys with no visible blue or other tint, in four elevation steps from darkest to lightest: recessed wells, the window background, panels, and raised controls. Surfaces that sit on each other SHALL be clearly distinguishable, with a contrast ratio of at least 1.2:1 between a panel and the window behind it, and between a panel and the wells and raised controls on it.

#### Scenario: Panels are neutral
- **WHEN** the app opens with no track loaded
- **THEN** the deck, mixer, sidebar and library backgrounds are neutral dark greys, panels a step lighter than the window behind them, with no blue-grey cast

#### Scenario: Panels and buttons stand out
- **WHEN** a deck panel is shown on the window background with its buttons and pads
- **THEN** the panel is visibly lighter than the window, and the buttons and pads are visibly lighter than the panel, each step at least 1.2:1 apart

### Requirement: Consistent top lighting
The system SHALL light every raised surface (panels, buttons, pads, fader caps, knob caps) from above. Each one has a lighter top edge, a darker bottom edge and a subtle top-to-bottom shading, so all controls appear lit from the same direction. The top-edge highlight SHALL be visible at normal viewing distance on every raised control.

#### Scenario: Pads and buttons share the light
- **WHEN** a deck's transport buttons and pads are shown side by side
- **THEN** each has a highlight along its top edge and a darker bottom edge, in the same direction as the panel they sit on

#### Scenario: Highlight is visible when unlit
- **WHEN** a deck is loaded and paused, with no pads or transport buttons lit
- **THEN** the top-edge highlight of each unlit button and pad is still clearly visible

### Requirement: Recessed screens and wells
The system SHALL draw readout screens (BPM, key, pitch, time), waveform lanes, overviews, fader slots and meters as recessed wells, darker than the surface around them with a shadow along their inner top edge. Readout screens SHALL also show a faint glass reflection that never reduces the readability of their text.

#### Scenario: Time screen looks recessed
- **WHEN** a deck shows its elapsed and remaining time
- **THEN** the screen is darker than the deck panel, shaded along its inner top edge, with a faint reflection over the readout

### Requirement: Crisp edges without soft shadows
The system SHALL use small corner radii (no more than 4px on panels and 3px on controls, apart from round controls such as knobs and transport buttons) and SHALL NOT use soft blurred drop shadows. Depth SHALL come from edges, shading and recessed wells.

#### Scenario: No floating cards
- **WHEN** the deck and mixer panels are shown
- **THEN** their corners are at most 4px and no blurred shadow surrounds them

### Requirement: Deck colour identity
The system SHALL mark each deck with its colour so the deck is identifiable at a glance even when idle: the deck panel's accent edge, its header and track title, its deck tag, its waveform tint and playhead marker, and toggles inside the deck that are switched on.

#### Scenario: Neutral deck panel chrome
- **WHEN** deck A has a track loaded and is paused
- **THEN** its panel's accent edge and heading are in deck A's colour, while its panel fill, section labels and general text stay neutral

#### Scenario: Deck panel shows its colour when idle
- **WHEN** the app opens with no track loaded
- **THEN** deck A's panel shows an accent edge and heading in deck A's colour, and deck B's in deck B's colour

#### Scenario: Switched-on toggle uses the deck colour
- **WHEN** deck B's pads are switched to LOOP mode
- **THEN** deck B's LOOP mode toggle is lit in deck B's colour, and the CUE toggle beside it is unlit

#### Scenario: State colours are kept
- **WHEN** deck A is playing with a hot cue set
- **THEN** deck A's play button is lit green and the hot cue pad pink, not deck A's colour

#### Scenario: Mixer split by side
- **WHEN** the mixer is shown
- **THEN** controls and labels for channel A use deck A's colour and those for channel B use deck B's colour, and the EQ knobs keep their band colours

### Requirement: Mixer channels in their deck's colour
In the mixer, the system SHALL show each channel's labels, fader fill and switched-on controls in the colour of the deck they control.

#### Scenario: Filter knobs follow their channel
- **WHEN** the mixer is shown
- **THEN** channel A's filter knob is in deck A's colour and channel B's filter knob in deck B's colour

### Requirement: State colours over deck colour
The system SHALL keep state colours (play, cue, hot cue, loop, meters, warning, danger) in their own colours and SHALL NOT replace them with the deck colour.

#### Scenario: Active loop keeps the loop colour
- **WHEN** deck B's pads are in LOOP mode and a beat loop is active
- **THEN** the active loop pad is lit in the loop colour, not in deck B's colour

### Requirement: Professional typography with steady readouts
The system SHALL render interface text in a bundled sans-serif UI font and numeric readouts (BPM, key, pitch, elapsed and remaining time, bar and beat, track list numbers) in a bundled monospace font. Every digit SHALL be the same width, so a readout does not shift sideways as its value changes. The fonts SHALL be bundled with the app, so the look is the same on every machine.

#### Scenario: Time readout holds still
- **WHEN** a deck plays and its elapsed time counts up from 0:01.1 to 0:11.1
- **THEN** the digits change in place without the readout shifting sideways

#### Scenario: Same look without installed fonts
- **WHEN** djkitty runs on a machine that has neither font installed
- **THEN** the interface and readouts use the bundled fonts

### Requirement: Readable contrast
The system SHALL keep primary text at a contrast ratio of at least 7:1 against the surface behind it, and secondary text and labels at least 4.5:1. This SHALL include surfaces tinted with a deck or accent colour, such as a selected row, a lit toggle or a primary button, and coloured text such as the key colour shown on a selected row.

#### Scenario: Dim labels stay readable
- **WHEN** small labels such as "ELAPSED", "TEMPO" or table column headers are shown
- **THEN** each has a contrast ratio of at least 4.5:1 against its background

#### Scenario: Selected row stays readable
- **WHEN** a track row is selected in the library
- **THEN** its title has a contrast ratio of at least 7:1 and its other columns, including the key, at least 4.5:1 against the selected row's background

#### Scenario: Lit toggle label stays readable
- **WHEN** a toggle is lit in a deck colour
- **THEN** its label has a contrast ratio of at least 4.5:1 against the lit background

### Requirement: Style changes never move the layout
The system SHALL keep every panel, control and text element at the same position and size in points as before the restyle, at every window size, so a restyle changes only how things are painted.

#### Scenario: Same layout before and after
- **WHEN** the app is shown at the default window size before and after the restyle
- **THEN** every panel and control occupies the same rectangle, and the same text sizes are used

### Requirement: Meaningful colours are unchanged
The system SHALL keep the colours that carry meaning (waveform band colours, cue, hot cue and loop colours, key colour, warning, danger and OK colours, and the two deck colours) recognisably the same hues, so nothing a DJ has learned to read changes meaning.

#### Scenario: Waveform bands unchanged
- **WHEN** a track's waveform is shown after the restyle
- **THEN** lows, mids and highs keep their blue, amber and mint colouring

### Requirement: Accent colour for shared areas
The system SHALL use a single accent colour, distinct from both deck colours and every state colour, to highlight selection and focus in areas that belong to neither deck: the selected track row, the active set in the sidebar, focused text fields and the primary button in a dialog. It SHALL NOT be used inside the deck panels or the mixer.

#### Scenario: Selected track row
- **WHEN** a track is selected in the library
- **THEN** its row is highlighted in the accent colour, not in grey and not in a deck colour

#### Scenario: Active set in the sidebar
- **WHEN** a set is open
- **THEN** its row in the sidebar is marked in the accent colour

#### Scenario: Primary dialog button
- **WHEN** the new set dialog is open with a valid name
- **THEN** its Save button is lit in the accent colour and Cancel is unlit
