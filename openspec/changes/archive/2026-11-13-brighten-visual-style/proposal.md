# Proposal

## Why

The "studio satin" restyle reads well but looks dull. Its surfaces are so close in lightness (window, panel and button are each about 1.1:1 apart) that they blend together, the top lighting is too faint to see on near-black, and colour only appears when something is active. With an empty or paused deck, nearly the whole window is grey, and toggles that are switched on just turn a slightly lighter grey. The app needs more colour and energy without losing the colours DJs rely on to read state.

## What Changes

- Widen the steps between the four surface levels (wells, window, panels, raised controls) so panels and controls clearly stand out, and make the top-edge lighting strong enough to see. Surfaces stay neutral grey with no tint.
- Give each deck a colour identity that is always visible: its panel gets an accent edge in the deck colour, and its header and title use that colour.
- Light toggles that are switched on inside a deck (the CUE/LOOP pad mode switch, the beatgrid edit chip and similar) in that deck's colour instead of a lighter grey.
- Split the mixer by side: each channel's controls and accents use the colour of the deck they control.
- Add one app accent colour, purple, for shared areas: the selected track row, the active set in the sidebar, focused text fields and primary dialog buttons.
- Keep the state colours unchanged: play (green), cue (yellow), hot cues (pink), loops (green), meters, warning and danger. Keep the waveform band colours unchanged.
- **BREAKING (visual rule)**: drop the rule that deck panel borders and headings must be neutral. Deck colour now marks the deck's frame and its "on" toggles as well as its state.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `visual-style`: the palette requirement gains clearly visible elevation steps; "Deck colour only where it carries meaning" becomes a deck colour identity rule covering frame, header and "on" toggles; a new requirement adds the purple accent for shared areas; the contrast requirement extends to text on accent-tinted surfaces such as the selected row.

## Impact

- `crates/djk-app/src/theme.rs`: new surface values, stronger edge light and sheen, a new accent token, and deck-tinted variants for frames and toggles. Contrast tests extended to the new surfaces.
- `crates/djk-app/src/widgets.rs`: `lit_button` and `lit_button_sized` take an accent colour for their "on" state instead of always using grey.
- `crates/djk-app/src/ui.rs`: `card()` gains an optional deck accent edge; deck header, pad-mode switch, grid chip, mixer strip, list rows, track table selection and dialogs use the new colours.
- No layout changes: the existing layout dump (`DJKITTY_DUMP_LAYOUT=1`) must stay identical. No changes to audio, library or controller code, and no new dependencies.
