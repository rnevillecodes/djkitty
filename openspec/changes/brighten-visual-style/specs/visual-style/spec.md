## RENAMED Requirements

- FROM: `### Requirement: Deck colour only where it carries meaning`
- TO: `### Requirement: Deck colour identity`

## MODIFIED Requirements

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

### Requirement: Deck colour identity
The system SHALL mark each deck with its colour so the deck is identifiable at a glance even when idle: the deck panel's accent edge, its header and track title, its deck tag, its waveform tint and playhead marker, and toggles inside the deck that are switched on. In the mixer, each channel's labels, fader fill and switched-on controls SHALL use the colour of the deck they control. State colours (play, cue, hot cue, loop, meters, warning, danger) SHALL keep their own colours and SHALL NOT be replaced by the deck colour.

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

## ADDED Requirements

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
