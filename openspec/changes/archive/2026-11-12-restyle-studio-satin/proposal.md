# Proposal

## Why

djkitty works well but looks toy-like. Its large rounded cards, soft blurred shadows, blue-grey slate fills, deck colours on every border and egui's default font read as cartoony next to the DJ software and hardware a learner aspires to. A consistent "studio satin" look makes the app feel like a serious instrument without adding visual noise. Satin here means near-black neutrals, surfaces lit from above, and recessed glassy screens.

## What Changes

- **Palette:** replace the blue-grey slate surfaces with near-black neutrals, with four elevation steps: window, panel, raised control, recessed well.
- **Lighting:** every raised surface (panels, buttons, pads, fader caps, knob caps, list rows on hover) is lit from above. It gets a 1px light top edge, a dark bottom edge and a subtle top-to-bottom gradient.
- **Recessed wells:** screens, waveform lanes, fader slots and meters become recessed wells with an inner shadow along the top. BPM, key and time screens also get a faint glass reflection.
- **Corners:** panels use 4px corners and controls 3px, instead of 10px and 6px. Soft blurred drop shadows are removed, and depth comes from edges instead.
- **Deck colours:** used only where they mean something: the deck tag, waveform tint and playhead marker, lit buttons and pads, deck fader fill and meters. Card borders, accent strips and headings go neutral. The deck badge becomes a compact tag.
- **Fonts:** a bundled UI font replaces egui's default, and a monospace font with even-width digits is used for BPM, key, pitch, time and bar readouts, so numbers don't jiggle as they change.
- **Whole app:** the library table, sidebar, track details, dialogs, menus, tooltips and status bar get the same palette, type and edges.
- **No layout changes:** layout structure, panel and widget dimensions, and text sizes stay as they are. Only how things are painted changes.
- Waveform band colours, hot cue and loop colours, and the meaning of every colour stay the same.

## Capabilities

### New Capabilities

- `visual-style`: the app's look as a contract: palette and elevation, lighting, corner radii, where deck colour may appear, typography and readouts, contrast, and the guarantee that a restyle does not move or resize anything.

### Modified Capabilities

None. Waveform colouring, layout and every control's behaviour are unchanged.

## Impact

- `djk-app` only:
  - `theme.rs`: new palette, lighting constants, egui `Visuals` and fonts.
  - `widgets.rs`: fader, knob, pad, transport button, meter, screen and gradient helpers.
  - `ui.rs`: `card`, the deck badge, the mixer and level panels, sidebar, library table, track details, dialogs and status bar.
  - `waveview.rs`: lane and overview wells and borders.
- **Fonts:** two open-licence (SIL OFL) font files added under `crates/djk-app/assets/fonts/` with their licence texts, embedded with `include_bytes!`. That adds roughly 1–2 MB to the binary.
- **Logo:** the logo artwork is unchanged. It sits in the status bar against the new neutrals.
- No engine, library, controller or behaviour changes. Screenshots at the default and minimum window sizes are the main verification.
