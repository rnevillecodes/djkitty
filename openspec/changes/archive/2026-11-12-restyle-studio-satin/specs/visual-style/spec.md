# Spec Delta

## Purpose

Defines how djkitty looks: a calm, professional "studio satin" style with near-black neutral surfaces lit from above. Colour is kept for things that carry meaning, so the controls read like real DJ equipment.

## ADDED Requirements

### Requirement: Neutral near-black palette
The system SHALL paint its surfaces in near-black neutral greys with no visible blue or other tint, in four elevation steps from darkest to lightest: recessed wells, the window background, panels, and raised controls.

#### Scenario: Panels are neutral
- **WHEN** the app opens with no track loaded
- **THEN** the deck, mixer, sidebar and library backgrounds are neutral dark greys, panels a step lighter than the window behind them, with no blue-grey cast

### Requirement: Consistent top lighting
The system SHALL light every raised surface (panels, buttons, pads, fader caps, knob caps) from above. Each one has a lighter top edge, a darker bottom edge and a subtle top-to-bottom shading, so all controls appear lit from the same direction.

#### Scenario: Pads and buttons share the light
- **WHEN** a deck's transport buttons and pads are shown side by side
- **THEN** each has a highlight along its top edge and a darker bottom edge, in the same direction as the panel they sit on

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

### Requirement: Deck colour only where it carries meaning
The system SHALL use each deck's colour only to identify that deck or show its state: the deck tag, waveform tint and playhead marker, lit transport buttons and pads, the deck's channel fader fill and meters, and selection highlights tied to the deck. Panel borders, accent strips, section headings and general text SHALL be neutral.

#### Scenario: Neutral deck panel chrome
- **WHEN** deck A has a track loaded and is paused
- **THEN** its panel border and heading text are neutral, and its colour appears only in its deck tag, its waveform and markers, and its lit controls

### Requirement: Professional typography with steady readouts
The system SHALL render interface text in a bundled sans-serif UI font and numeric readouts (BPM, key, pitch, elapsed and remaining time, bar and beat, track list numbers) in a bundled monospace font. Every digit SHALL be the same width, so a readout does not shift sideways as its value changes. The fonts SHALL be bundled with the app, so the look is the same on every machine.

#### Scenario: Time readout holds still
- **WHEN** a deck plays and its elapsed time counts up from 0:01.1 to 0:11.1
- **THEN** the digits change in place without the readout shifting sideways

#### Scenario: Same look without installed fonts
- **WHEN** djkitty runs on a machine that has neither font installed
- **THEN** the interface and readouts use the bundled fonts

### Requirement: Readable contrast
The system SHALL keep primary text at a contrast ratio of at least 7:1 against the surface behind it, and secondary text and labels at least 4.5:1.

#### Scenario: Dim labels stay readable
- **WHEN** small labels such as "ELAPSED", "TEMPO" or table column headers are shown
- **THEN** each has a contrast ratio of at least 4.5:1 against its background

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
