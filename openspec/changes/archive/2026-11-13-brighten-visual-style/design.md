# Design

## Context

All colours already go through named tokens in `crates/djk-app/src/theme.rs`. Raised and recessed surfaces are painted by `sheen.rs`, and the custom controls (pads, transport buttons, faders, knobs, lit buttons) are in `widgets.rs`. So this change is mostly new token values plus a handful of call sites, not a restructure.

Measured today (WCAG contrast ratio):

| Pair | Ratio |
|------|-------|
| `BG` vs `PANEL` | 1.09:1 |
| `WELL` vs `PANEL` | 1.12:1 |
| `PANEL` vs `RAISED` | 1.14:1 |
| `RAISED` vs `RAISED_HOVER` | 1.16:1 |
| `TEXT_DIM` vs `PANEL` | 6.69:1 |

Relevant current behaviour:

- `lit_button` / `lit_button_sized` (`widgets.rs`) take `selected: bool`, and "selected" paints `CAP` grey. They're used for the pad mode switch (deck toggle), the dialog Save buttons (primary action) and many plain buttons.
- The beatgrid `GRID` chip (`ui.rs`, `grid_tools`) bypasses `lit_button` and fills itself with the deck colour while editing. It's the only toggle that is already deck-coloured.
- `card()` (`ui.rs`) paints every panel with `sheen::raised_shapes` and a `HAIRLINE` outline. It has no accent parameter, which the studio satin restyle removed on purpose.
- Pads already light in their own colour with `mix(RAISED, colour, 0.8)` and dark ink (`theme::BG`). That's a proven pattern for lit fills.
- The library table selection comes from egui's `visuals.selection.bg_fill` (via `row.set_selected`). Text field focus uses `visuals.selection.stroke`. `list_row` (sidebar sets) paints its own selected state: a raised `RAISED_HOVER` fill with a white bar.
- The mixer already shows deck colour in its A/B labels and channel fader fills. The filter knobs are neutral (`TEXT`), and the EQ knobs use the band colours.
- `DJKITTY_DUMP_LAYOUT=1` (`layout_dump.rs`) logs every panel and control rect. The previous restyle used it, with screenshots, to prove that nothing moved.

## Goals / Non-Goals

**Goals:**
- Meet the new `visual-style` requirements with the smallest set of new tokens.
- One code path for "lit in a colour", shared by pads, toggles and primary buttons.
- Every contrast rule in the spec enforced by unit tests, not by eye.

**Non-Goals:**
- Tinted (non-neutral) surfaces. They were considered and not chosen.
- A light theme or user-selectable themes.
- Changing transport buttons, pads, meters, waveform or LED colours.
- Any layout, size or font change.

## Decisions

### 1. Surface ladder: brighten panels and controls, keep wells and window near black

Near black, the two darkest levels can't be 1.2:1 apart (pure black against today's `BG` is only 1.09:1). The spec therefore asks for 1.2:1 only between surfaces that sit on each other: window|panel, panel|well and panel|raised. That lets `WELL` and `BG` stay near black while `PANEL` and `RAISED` move up.

Starting values, close to the mockup the user approved:

| Token | Now | Starting value | Check |
|-------|-----|----------------|-------|
| `WELL` | `#08090a` | `#050607` | vs `PANEL` 1.24:1 |
| `BG` | `#0d0e10` | `#08090a` | vs `PANEL` 1.22:1 |
| `PANEL` | `#17181b` | `#1e2024` | |
| `RAISED` | `#222428` | `#303338` | vs `PANEL` 1.29:1 |
| `RAISED_HOVER` | `#2d2f34` | `#3b3e44` | |
| `HAIRLINE`, `CAP`, `CAP_HOT`, `LED_OFF`, `faint_bg_color`, widget strokes | | raised by the same amount, so their order is kept | |
| `TEXT_DIM` | `#9b9fa7` | about `#a9adb5` | today's value is only 4.04:1 on the new `RAISED_HOVER`; the new one gives 4.76:1 |

These are starting points. The final values are set in the screenshot pass (task 6), and the tests hold the floor. A new unit test checks the 1.2:1 pairs, in the same style as the existing contrast tests.

Alternative considered: raising every level evenly, wells included. That would push `PANEL` and `RAISED` well into mid-grey to keep 1.2:1 between every pair, and would lose the "near-black" character the spec keeps.

### 2. Stronger top light

`EDGE_LIGHT` goes from about 7% to about 16% white, `SHEEN` from 0.05 to about 0.08, and `PANEL_SHEEN` from 0.015 to about 0.025. Pads already use `SHEEN * 1.5`, so they scale up with it. The final values are tuned by screenshot. The spec's "highlight is visible when unlit" scenario is checked by eye at 100% scale. It isn't a contrast number.

### 3. One "lit in a colour" path: `lit_button(ui, enabled, lit: Option<Color32>, button)`

`selected: bool` becomes `lit: Option<Color32>` in `lit_button` and `lit_button_sized`. `Some(c)` paints the same lit fill as a pad (`mix(RAISED, c, 0.8)`, darker when pressed) and switches the label to the dark ink pads use. `None` keeps the existing unlit, hover and pressed looks.

- Pad mode switch: `lit: on.then_some(deck_colour(deck))`.
- `GRID` chip: moves onto `lit_button` with the deck colour, so it gets the shared lighting and pressed state instead of its own flat fill.
- Dialog Save buttons: `Some(theme::ACCENT)`.
- Every other caller: `None`.

Labels inside lit buttons currently set their own `RichText` colour (`TEXT` when on). To avoid every call site having to know the ink, `paint_lit` returns the ink colour to use and callers stop hard-coding it. Where that isn't practical, the caller passes `theme::BG` when lit.

Alternatives considered: a separate `toggle()` widget, which would duplicate the lit/hover/pressed logic `paint_lit` already has; or keeping `bool` plus a colour parameter, which allows the meaningless combination "not selected but has a colour".

### 4. Deck accent edge on deck cards, in a muted deck colour

`card()` gets an `accent: Option<Color32>` parameter. When it's set, a 2px line in that colour is painted inside the top edge, after the panel is laid out. It's painted, not added as a margin, so the layout dump stays identical. Deck panels and waveform lanes pass their deck's frame colour. The mixer passes `None` (see Decision 5) and the library and sidebar pass `None`.

The frame uses `theme::deck_frame(deck)`, a muted version of the deck colour (`mix(PANEL, deck_colour, ~0.7)`). The full deck colour is kept for lit toggles, the deck tag and the title. This resolves the open question from the discussion: deck B's orange (`#ff8a3d`) sits within about 12° of hue of the waveform mids (`#f0a85c`) and `WARN` (`#f5a63c`). Muting the frame keeps all three hues unchanged, as the "Meaningful colours are unchanged" requirement asks, and leaves full-strength orange to mean "on". The header track title uses the full deck colour. Artist and other secondary text stay `TEXT_DIM`.

Alternatives considered: shifting `DECK_B` towards red-orange, which moves it towards `DANGER`; or shifting `BAND_MID` towards gold, which changes a waveform look the user said they like. Both change "meaningful" hues and would need a spec change. If the muted frame still clashes in the screenshot pass, that's a question for the user, not a tuning decision.

### 5. Mixer split by side

The mixer card stays one panel. Its top accent edge is painted in two halves, deck A's frame colour on the left and deck B's on the right, split at the crossfader centre. `card()` takes the accent as a small enum (`None`, `Single(c)`, `Split(a, b)`) so the mixer doesn't need special-case painting. Each channel's filter knob changes from `TEXT` to its deck colour. The A/B labels and fader fills already use the deck colour, and the EQ knobs keep their band colours (red when killed). The crossfader and master controls are shared and stay neutral.

### 6. App accent `ACCENT` for shared areas, separate from `KEY`

A new token `ACCENT` (starting value a bluer violet, about `#8f6bff`) is used for selection and focus outside the decks and mixer:

- `visuals.selection.bg_fill` becomes a low-strength accent tint on `BG` (about `mix(BG, ACCENT, 0.2)`). This covers the library table's selected row and text selection.
- `visuals.selection.stroke` becomes `ACCENT`, which gives focused text fields an accent outline.
- `list_row` (sidebar sets): the selected fill becomes the same accent tint, the white bar becomes an `ACCENT` bar, and the icon tile becomes accent-lit.
- Dialog primary buttons: `lit_button(..., Some(ACCENT), ...)` (Decision 3).

`ACCENT` is kept separate from `KEY` (`#b78cff`). They're in the same family, but `KEY` is used only as text colour while `ACCENT` is used for fills, edges and outlines, so the two roles stay distinct. `KEY` text on a selected row measures about 5.6–5.9:1 on candidate tints, which passes the 4.5:1 floor. A test locks that in (Decision 7).

Alternative considered: reusing `KEY` itself as the accent. It's simpler, but the key column's text would sit on a fill of the same hue and lose contrast.

### 7. Contrast tests cover every new pairing

The tests in `theme.rs` are extended to cover:

- 1.2:1 for `BG|PANEL`, `WELL|PANEL` and `PANEL|RAISED`.
- The existing 7:1 and 4.5:1 text checks, against the new surfaces plus the selected-row tint.
- `TEXT_DIM` and `KEY` at least 4.5:1, and `TEXT` at least 7:1, on the selected-row tint.
- Dark lit ink at least 4.5:1 on the lit fill (`mix(RAISED, c, 0.8)`) for `DECK_A`, `DECK_B`, `ACCENT`, `HOT_CUE`, `LOOP` and `OK`. This also guards the pads, which have never been tested.
- A negative test, like the existing `the_check_catches_a_too_dark_label`, showing that the step check fails on today's values.

The lit-fill mix moves to a single `theme::lit_fill(colour, pressed)` helper used by pads, lit buttons and tests, so the tests check the real formula.

### 8. Verification follows the previous restyle

Same approach as `restyle-studio-satin`: capture a baseline layout dump and screenshots at 1280×760 and 960×600 before any change, keep the dump identical throughout, and finish with before and after screenshots for a tuning pass with the user. The baseline is kept in the change folder and removed when the user is happy.

## Risks / Trade-offs

- [Too much colour competing: blue and orange frames, pink pads, green play and loops, yellow cue, band-coloured waveforms] → Frames use the muted deck colour. Full deck colour appears only on the deck tag, title and switched-on toggles. State colours are untouched. The final screenshot pass checks a busy state (both decks loaded, one playing, hot cues set, a loop active, LOOP mode on) for clashes.
- [Deck B's orange "on" toggle next to an amber `WARN` or a yellow lit Cue] → They sit in different places (toggles are on the pad row, Cue is on the transport, warnings are in the status bar). Checked in the screenshot pass. If it's still confusing, raise it with the user before changing any hue.
- [Brighter UI is harsher in a dark booth] → Wells, the window and the waveform lanes stay near black. Only panels and controls brighten, by roughly a third. The values are tuned with the user.
- [Lit fills too bright for dark ink, or too dark for it] → The ink contrast test in Decision 7 covers every lit colour.
- [Changing `lit_button`'s signature touches many call sites] → It's a mechanical change the compiler checks. Only three callers pass a colour.
- [egui's `selection.bg_fill` is also used for text selection in fields] → A purple tint text selection is acceptable and consistent. Checked by screenshot of a focused field with selected text.

## Migration Plan

There's no data or settings migration. Rolling back means reverting the commit. The studio satin baseline isn't needed to roll back.
