# Design

## Context

- **Where the look lives:** all painting is in `djk-app`. Colours are constants in `theme.rs`, which also sets egui `Visuals` and spacing. Custom controls (fader, knob, pad, transport button, meter, screen) are hand-painted in `widgets.rs`. Panels go through `card()` in `ui.rs`, which draws a 10px-radius slate panel with a blurred `Shadow` and a deck-coloured strip. Waveform lanes and overviews are painted in `waveview.rs`, with deck-coloured lane borders. Everything else (buttons, combo boxes, text fields, the library table, menus, dialogs) is stock egui styled through `Visuals`.
- **Colour literals:** ~70 raw `Color32` values sit outside `theme.rs` (35 in `widgets.rs`), which is why the look drifts.
- **Fonts:** egui's built-in fonts are the only ones used. Readouts already ask for `FontId::monospace`, so swapping the monospace family steadies them without touching call sites.
- **Rendering limits:** egui (0.37, wgpu) gives filled shapes, strokes, meshes with per-vertex colour, and one blurred `Shadow`. It has no gradient fills on rounded rects and no shaders without a custom paint callback. `widgets::vertical_gradient` already builds gradients from a mesh.
- **Constraint from the user:** layout structure and all dimensions stay as they are.

## Goals / Non-Goals

**Goals:**
- One small set of style primitives (`raised`, `well`, `glass`, `round_lit`) and named tokens, used everywhere, so the look stays consistent and is easy to tune.
- Remove raw colour literals from `ui.rs`, `widgets.rs` and `waveview.rs`. Colours come from `theme.rs`.
- Pixel-identical layout. The only differences in a before/after screenshot are colours, shading, corner shapes and glyph shapes.

**Non-Goals:**
- Light mode, user-selectable themes, animation, new icons or a new logo.
- Custom shaders or a wgpu paint callback.
- Changing any size, spacing, font size or layout constant (`DECKS_ROW_HEIGHT`, lane heights, column widths, `item_spacing` and so on).

## Decisions

### 1. Tokens in `theme.rs`

The palette is named by role, not by widget:

| Token | Value (start point, tune by eye) | Use |
|---|---|---|
| `WELL` | `#08090a` | screens, lanes, fader slots, meters, text fields |
| `BG` | `#0d0e10` | window, library, status bar |
| `PANEL` | `#16181b` | deck and mixer panels, sidebar, details, dialogs |
| `RAISED` | `#1e2024` | buttons, pads, caps, hovered rows |
| `EDGE_LIGHT` | white ~7% alpha | 1px top highlight on raised surfaces |
| `EDGE_DARK` | black ~55% alpha | 1px bottom edge and outlines |
| `HAIRLINE` | `#26282c` | dividers, table grid, neutral borders |
| `TEXT` / `TEXT_DIM` / `HEADING` | neutral greys, checked for contrast (Decision 7) | text |
| `R_PANEL` = 4, `R_CONTROL` = 3 | | corner radii |
| `SHEEN` | ~4% | how much lighter a raised surface's top is than its bottom |

The meaning colours stay: `DECK_A`/`DECK_B`, the band colours, `CUE_MARKER`, `HOT_CUE`, `LOOP`, `KEY`, `WARN`, `DANGER`, `OK`. Only their brightness may be nudged for contrast on the darker background. Old names that become misleading (`CARD`, `CARD_BORDER`, `TOP_BG`) are removed rather than aliased, so the compiler finds every use.

*Alternative:* keep the existing names and only change their values. Rejected: `CARD` and `TOP_BG` describe the old structure, and a rename is what flushes out the raw literals.

### 2. Style primitives in a new `sheen.rs`

- `raised(painter, rect, radius, base)`: a rounded rect filled with a vertical gradient from `base` lightened by `SHEEN` to `base`, then a 1px `EDGE_LIGHT` line along the top (inset by the radius) and a 1px `EDGE_DARK` outline.
- `well(painter, rect, radius)`: `WELL` fill, a ~6px inner shadow fading from black at the top edge, and a faint 1px light line along the bottom edge (light catching the lip).
- `glass(painter, rect)`: a translucent white (~3–4% alpha) shape covering the upper part of a screen, cut by a gentle diagonal, painted over the background and under the text.
- `round_lit(painter, centre, r, base)`: a circle with the same vertical gradient, a top rim highlight arc and a dark outline, for knob caps and transport buttons.

A rounded gradient is drawn as a mesh: generate the rounded-rect outline points (corner arcs of a few segments each), fan-triangulate from the centre, and colour each vertex by its y position. Mesh edges aren't anti-aliased, so every gradient body is finished with a 1px anti-aliased stroke (the `EDGE_DARK` outline), which hides the stair-steps.

*Alternative:* a pre-rendered gradient texture stretched over a rounded rect. Rejected: it adds texture management for no visible gain at these sizes.

### 3. Panels without shadows

`card()` loses the `Shadow`, the 10px radius and the accent strip. It paints `raised(PANEL)` with `R_PANEL` (a very low `SHEEN` for large panels, so they read as satin rather than buttons) and a `HAIRLINE` border. Its `accent` parameter goes, and call sites stop passing deck colours. The frame's inner margins stay the same, so content doesn't move. The top section's `TOP_BG` becomes `BG`, and depth comes from the panels being one step lighter.

### 4. Deck identity

- **Deck badge:** the 34px circle becomes a compact tag in the same 34px slot: a `RAISED` rounded rect with the letter in deck colour and a 2px deck-coloured underline. Nothing moves.
- **Waveform lanes:** the deck-coloured lane borders become neutral wells. The letter in the lane corner stays in deck colour, which is what tells the stacked lanes apart.
- **Unchanged deck colour:** lit transport rings, lit pads, the channel fader fill, meters, the playhead and position markers, and the drop-target outline while dragging. All of these show deck identity or state.

### 5. Controls

Sizes and hit areas stay as they are. Only painting changes:

- **Fader:** the slot is a `well`, the cap is `raised(RAISED)` with grip lines, and the indicator line is white when hovered and neutral light grey otherwise. The fill stays deck-coloured on channel faders.
- **Knob:** the cap uses `round_lit`. The arc track is a `well`-dark ring, and the value arc keeps its current colours.
- **Transport buttons:** the body uses `round_lit`. The LED ring and glow keep their colour but only glow when lit.
- **Pads:** unlit pads are `raised(RAISED)` with `R_CONTROL`. Lit pads are a saturated fill of their colour with the same top highlight and a slightly darker bottom.
- **Meters:** the housing is a `well`. Unlit segments are very dim, so the meter reads as off rather than as grey blocks.
- **Screens:** `well` plus `glass`.
- **Stock egui buttons** (Import, Add, CUE/LOOP switch, Shortcuts, grid tools, dialog buttons): a `widgets::button` helper paints `raised` behind a transparent-fill `egui::Button` (the same placeholder-shape trick `card()` uses). Call sites switch from `egui::Button`/`ui.button` to the helper. Combo boxes and menus keep egui's widgets, restyled through `Visuals` (flat `RAISED`, `HAIRLINE` strokes, `R_CONTROL`).
- **Text fields:** `Visuals.extreme_bg_color = WELL` with `HAIRLINE` strokes, so they read as recessed.

### 6. Fonts

Two SIL Open Font Licence families, embedded with `include_bytes!` from `crates/djk-app/assets/fonts/` alongside their `OFL.txt` licence files:

- **Inter:** the proportional (UI) family.
- **JetBrains Mono:** the monospace family. Every digit is the same width, it's clear at small sizes, and it has a slashed or dotted zero.

egui's built-in fonts stay as fallbacks in both families, so symbols the new fonts lack (such as `−`, `×`, `…` and arrows) still render. Bold text keeps using egui's `strong` (brighter colour), so no second weight is required.

To keep dimensions the same, each new font gets a `FontTweak` scale so that its row height at every size used matches the old font's within 1/32 pt. egui rounds ascent and descent separately to that step, so no single scale matches every size exactly. That residue is far below a pixel, and the layout dump confirms it moves nothing. The cost is that glyphs render at 95% (Inter) and 88% (JetBrains Mono) of their nominal size, which is why the readouts look slightly smaller. Text sizes in points are unchanged everywhere.

*Alternative:* load system fonts (Segoe UI, Consolas) at run time. Rejected: Windows-only, not redistributable, and the spec requires the same look on every machine.

### 7. Contrast checked in a test

A unit test in `theme.rs` computes the WCAG contrast ratio of `TEXT`, `HEADING` and `TEXT_DIM` against `BG`, `PANEL`, `RAISED` and `WELL`, and asserts 7:1 and 4.5:1 (the `visual-style` spec, "Readable contrast"). Tuning the palette by eye can then never quietly break readability.

### 8. Verifying "no layout changes"

Before any painting change, capture reference screenshots at 1280×760 and 960×600 with `PrintWindow` (flag 2) on the app window, which works for this wgpu window where a screen copy comes back black. After each group of work, capture the same views. An edge-detected difference of the two images should show no shifted outlines, only colour changes.

As a second check, a debug-only env var (`DJKITTY_DUMP_LAYOUT=1`) logs the rect of every top-level panel and custom control once. Comparing the logs before and after must give identical numbers. This guards the "dimensions stay the same" constraint mechanically, beyond what screenshots catch. The same env var also prints frame times every 120 frames, which is how the 60 fps check is made.

## Risks / Trade-offs

- [New fonts change text widths and line heights, nudging layout] → `FontTweak` matches row heights (Decision 6). The layout dump and screenshot comparison catch drift. Truncation of long titles may land on a different character, which is acceptable as the boxes don't move.
- [Mesh gradients look jagged on corners] → Every gradient gets an anti-aliased 1px outline. Radii are small (3–4px), so the stair-steps are hidden by the stroke.
- [Too much sheen looks kitsch] → `SHEEN`, `EDGE_LIGHT` and glass alpha are single tokens. Start subtle (~4%) and tune from screenshots together.
- [Darker background makes the deck colours look harsher] → Allow small brightness or saturation nudges to `DECK_A`/`DECK_B` within the same hue. The spec requires the hues stay recognisably the same.
- [Binary size grows by ~1–2 MB with the fonts] → Acceptable for a desktop app. Subset the fonts to Latin plus the symbols used if it matters later.
- [Getting the font files means downloading from the upstream releases (rsms/inter, JetBrains/JetBrainsMono)] → The task asks the user before downloading and records the exact versions and sources in `assets/fonts/README.md`.
- [Painting more gradients costs frame time] → These are tens of small meshes per frame, negligible next to the waveform meshes. Check that the decks still repaint at 60 fps while playing.

## Open Questions

- Exact token values (greys, `SHEEN`, glass alpha) are tuned by eye from screenshots during implementation. They start from the table in Decision 1, and this doesn't change the approach or the tasks.
