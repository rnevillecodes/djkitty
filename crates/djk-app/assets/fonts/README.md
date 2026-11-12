# Bundled fonts

Both fonts are licensed under the SIL Open Font License 1.1 (see the `*-OFL.txt` files).
They are embedded unmodified in the djkitty binary (`theme::fonts`).

| File                        | Font              | Version | Source |
|-----------------------------|-------------------|---------|--------|
| `Inter-Regular.ttf`         | Inter (UI text)   | 4.1     | `extras/ttf/Inter-Regular.ttf` from https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip |
| `Inter-OFL.txt`             | Inter licence     | 4.1     | `LICENSE.txt` from the same zip |
| `JetBrainsMono-Regular.ttf` | JetBrains Mono (readouts) | 2.304 | https://raw.githubusercontent.com/JetBrains/JetBrainsMono/v2.304/fonts/ttf/JetBrainsMono-Regular.ttf |
| `JetBrainsMono-OFL.txt`     | JetBrains Mono licence | 2.304 | https://raw.githubusercontent.com/JetBrains/JetBrainsMono/v2.304/OFL.txt |

Each font is scaled (`INTER_SCALE`, `MONO_SCALE` in `theme.rs`) so its rows are as tall as
egui's default fonts at every size, within 1/32 pt; `theme::tests::bundled_fonts_keep_row_heights`
checks this, and the ignored `probe_font_scales` test helps retune after a font update.
