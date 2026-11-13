# Tasks

## 1. Baseline

- [x] 1.1 Before any change, capture the layout dump (`DJKITTY_DUMP_LAYOUT=1`) and screenshots at 1280×760 and 960×600 with `PrintWindow` (flag 2) into `openspec/changes/brighten-visual-style/baseline/`, in two states: idle (no tracks loaded) and busy (both decks loaded, one playing, a hot cue set, a loop active, LOOP mode on one deck, a library row selected). Verify the dump and all four PNGs exist and show the current look

## 2. Surfaces, light and lit fills

- [x] 2.1 Set the new surface ladder and raise `TEXT_DIM` in `theme.rs` (design Decision 1), moving `HAIRLINE`, `CAP`, `CAP_HOT`, `LED_OFF`, `faint_bg_color` and the widget strokes in `theme::apply` up to keep their order. Add the 1.2:1 step test for `BG|PANEL`, `WELL|PANEL` and `PANEL|RAISED` with a negative test showing today's values fail it. Verify `cargo test -p djk-app theme` passes, including the existing 7:1 and 4.5:1 text tests against the new surfaces
- [x] 2.2 Strengthen `EDGE_LIGHT`, `SHEEN` and `PANEL_SHEEN` (design Decision 2). Verify by a screenshot of a loaded, paused deck at 100% scale where every unlit button and pad shows a clear top highlight, and an unchanged layout dump
- [x] 2.3 Add `theme::lit_fill(colour, pressed)` and switch `widgets::pad` to it (design Decision 7). Add the ink contrast test (dark lit ink at least 4.5:1 on `lit_fill` for `DECK_A`, `DECK_B`, `HOT_CUE`, `LOOP` and `OK`). Verify the test passes and pads look unchanged in a screenshot apart from the new surfaces

## 3. Deck colour identity

- [x] 3.1 Change `lit_button` and `lit_button_sized` from `selected: bool` to `lit: Option<Color32>`, painting `lit_fill` and the dark ink when lit (design Decision 3). Update every caller to `None` except the pad mode switch (deck colour) and the dialog Save buttons (left on `None` until 5.2). Verify `cargo build` and `cargo clippy --workspace --all-targets -- -D warnings` pass, and by screenshot that LOOP mode lights in the deck's colour with a readable label while CUE is unlit
- [x] 3.2 Move the `GRID` chip onto `lit_button` with the deck colour while editing (design Decision 3). Verify by screenshot that the chip lights in the deck colour with lighting and pressed state matching the pad mode switch, and an unchanged layout dump
- [x] 3.3 Add `theme::deck_frame(deck)` and the `card()` accent parameter (`None`, `Single`, `Split`), painting a 2px top edge inside the panel (design Decisions 4 and 5). Pass `Single(deck_frame)` for deck panels and waveform lanes and `None` for the library, sidebar and details. Colour the deck header track title in the full deck colour. Verify by idle and busy screenshots that each deck shows its frame colour and the layout dump is unchanged
- [x] 3.4 Split the mixer: pass `Split(deck_frame(A), deck_frame(B))` for the mixer card and colour each filter knob in its deck colour, leaving EQ, crossfader and master neutral or band-coloured as before (design Decision 5). Verify by screenshot that channel A's side reads blue and channel B's orange, and an unchanged layout dump
- [x] 3.5 Update the `## Purpose` of `openspec/specs/visual-style/spec.md` so it no longer says colour is kept only for meaning: decks carry their colour as identity, and shared areas use one accent. Verify the purpose matches the delta's requirements and `openspec validate --specs --strict` passes

## 4. Contrast for tinted surfaces

- [x] 4.1 Add `ACCENT` and the selected-row tint to `theme.rs`, with tests for `TEXT` at least 7:1, and `TEXT_DIM` and `KEY` at least 4.5:1, on the selected-row tint, and the ink test from 2.3 extended to `ACCENT` (design Decisions 6 and 7). Verify `cargo test -p djk-app theme` passes

## 5. Accent colour for shared areas

- [x] 5.1 Set `visuals.selection.bg_fill` to the selected-row tint and `visuals.selection.stroke` to `ACCENT`, and restyle `list_row`'s selected state with the tint, an `ACCENT` bar and an accent-lit icon tile (design Decision 6). Verify by screenshots of a selected library row (key column readable), the open set in the sidebar, and a focused search field with some text selected
- [x] 5.2 Pass `Some(theme::ACCENT)` for the Save buttons in the new set dialog (and any other dialog primary action). Verify by screenshot of the new set dialog with a valid name: Save is lit purple with a readable label, Cancel is unlit

## 6. Final check

- [x] 6.1 Compare the final layout dump and idle and busy screenshots at both window sizes with the baseline from 1.1. Verify the dump is identical, every `visual-style` scenario in the delta holds, deck B's orange frame and toggles are distinguishable from the waveform mids, `WARN` and the lit Cue, and `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass. Then share the before and after screenshots with the user for a tuning pass on the surface values, sheen, frame strength and accent, raising any hue clash with them rather than changing a meaningful colour
- [ ] 6.2 Once the user is happy, remove the `baseline/` folder (and `after/`) from the change. Verify it is gone and nothing references it

## Workflow follow-up

- Archive the change with `/opsx:archive` after review, so the `visual-style` delta merges into the main spec.
