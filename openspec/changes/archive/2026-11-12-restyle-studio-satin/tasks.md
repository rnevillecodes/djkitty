# Tasks

## 1. Baseline and guard rails

- [x] 1.1 Add the debug-only `DJKITTY_DUMP_LAYOUT=1` env var that logs, once after the first full frame, the rect of every top-level panel (waveform lanes, deck cards, levels, mixer, master, sidebar, library, details, status bar) and every custom control (faders, knobs, transport buttons, pads, screens, meters) (design Decision 8). Verify by running with it set and getting a stable, sorted log, identical across two runs
- [x] 1.2 Before any style change, capture reference screenshots at 1280×760 and 960×600 with `PrintWindow` (flag 2) and save the layout dump as the baseline in the change folder (`openspec/changes/restyle-studio-satin/baseline/`). Verify both PNGs and the dump exist and show the current look

## 2. Tokens, contrast and fonts

- [x] 2.1 Rework `theme.rs` into the role-named tokens of design Decision 1 (`WELL`, `BG`, `PANEL`, `RAISED`, `EDGE_LIGHT`, `EDGE_DARK`, `HAIRLINE`, text greys, `R_PANEL`, `R_CONTROL`, `SHEEN`), removing `CARD`, `CARD_BORDER` and `TOP_BG`, and update egui `Visuals` (window, widgets, selection, text fields as wells, menus and tooltips with `R_CONTROL` and hairline strokes, no window shadow). Verify the app builds and the layout dump is unchanged
- [x] 2.2 Add the WCAG contrast unit test for text tokens against every surface (design Decision 7). Verify it passes at 7:1 for primary text and 4.5:1 for secondary text and labels, and fails if `TEXT_DIM` is set to a too-dark grey
- [x] 2.3 Ask the user before downloading. Then add Inter (UI) and JetBrains Mono (monospace) with their `OFL.txt` licences and a `README.md` recording versions and sources under `crates/djk-app/assets/fonts/`, install them in `theme::apply` with egui's fonts as fallbacks, and tune each `FontTweak` so row heights match the old fonts (design Decision 6). Verify the layout dump is unchanged, readouts are monospace, and `−`, `×` and `…` still render

## 3. Style primitives

- [x] 3.1 Add `sheen.rs` with `raised`, `well`, `glass` and `round_lit` (design Decision 2), including the rounded-gradient mesh with an anti-aliased outline. Verify with unit tests that the gradient mesh's top vertices are lighter than its bottom ones and that all vertices stay inside the rect, and with a screenshot of a scratch panel showing smooth corners with no stair-steps

## 4. Decks, mixer and waveforms

- [x] 4.1 Restyle `card()` (no shadow, `R_PANEL`, `raised(PANEL)`, hairline border, no accent strip, no `accent` parameter), set the top section to `BG`, and replace the deck badge with the compact tag (design Decisions 3 and 4). Verify by screenshot and an unchanged layout dump
- [x] 4.2 Restyle the custom controls in `widgets.rs` (fader, knob, transport button, pad, meter, screen) with the primitives, removing their raw colour literals in favour of theme tokens (design Decision 5). Verify by screenshot with tracks loaded on both decks, one playing (lit play, meters moving, a hot cue set and a loop active), and an unchanged layout dump
- [x] 4.3 Make waveform lanes and overviews neutral wells with the deck letter as the only deck colour, keeping band colours, markers and the playhead unchanged (design Decision 4). Verify by screenshot that both lanes are distinguishable by their letter, band colours look as before, and the waveform still scrolls at 60 fps

## 5. Library, sidebar, details, dialogs and status bar

- [x] 5.1 Add the `widgets::button` lit-button helper and switch the app's buttons (Import, Add, CUE/LOOP switch, grid tools, dialog buttons, Shortcuts) to it, keeping their sizes (design Decision 5). Verify by screenshot and an unchanged layout dump
- [x] 5.2 Restyle the sidebar, set list rows, library table (header, rows, hover and selection in neutral greys, column dividers in `HAIRLINE`, missing-track styling kept), track details tiles, the new-set and remove dialogs, the shortcuts window, the drop overlay and the status bar with the tokens. Remove remaining raw colour literals from `ui.rs`. Verify by screenshots of each (including a dialog open and a row selected) and an unchanged layout dump

## 6. Final check

- [x] 6.1 Compare the final screenshots and layout dump with the baseline from 1.2 at both window sizes. Verify that the dump is identical, that edge-detected screenshot differences show no moved outlines, that every `visual-style` spec scenario holds, and that `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass. Then share the before and after screenshots with the user for a final tuning pass on the sheen
- [x] 6.2 Remove the `baseline/` folder from the change once the user is happy (it is reference material, not part of the product). Verify it is gone and nothing references it
