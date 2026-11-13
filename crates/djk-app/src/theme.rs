//! djkitty's "studio satin" theme: near-black neutral surfaces lit from above, colour kept
//! for what it means (decks, waveform bands, cues, LEDs).
//!
//! Surfaces come in four depths, darkest first: [`WELL`] (recessed: screens, lanes, fader
//! slots), [`BG`] (the window), [`PANEL`] (decks, mixer, sidebar) and [`RAISED`] (buttons,
//! pads, caps). Raised surfaces are lit from the top: see `crate::sheen`.

use egui::{Color32, CornerRadius, Stroke, Visuals};

use djk_core::DeckId;

// ---- Surfaces (neutral, no tint)

/// Recessed wells: readout screens, waveform lanes, fader slots, meters, text fields.
pub const WELL: Color32 = Color32::from_rgb(0x05, 0x06, 0x07);
/// The window: library, status bar, and behind the deck and mixer panels.
pub const BG: Color32 = Color32::from_rgb(0x08, 0x09, 0x0a);
/// Panels: decks, mixer, sidebar, track details, dialogs.
pub const PANEL: Color32 = Color32::from_rgb(0x1e, 0x20, 0x24);
/// Raised controls: buttons, pads, fader and knob caps.
pub const RAISED: Color32 = Color32::from_rgb(0x30, 0x33, 0x38);
/// Raised controls under the pointer, and selected rows.
pub const RAISED_HOVER: Color32 = Color32::from_rgb(0x3b, 0x3e, 0x44);
/// Dividers, table lines and neutral borders.
pub const HAIRLINE: Color32 = Color32::from_rgb(0x38, 0x3b, 0x41);
/// Fader and knob caps: a step above [`RAISED`] so they stand out on their panel.
pub const CAP: Color32 = Color32::from_rgb(0x44, 0x47, 0x4d);
/// Caps under the pointer or being dragged.
pub const CAP_HOT: Color32 = Color32::from_rgb(0x52, 0x55, 0x5c);
/// Grip grooves cut into caps, and other fine dark details.
pub const GROOVE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 100);
/// The caution band of level meters (between OK green and danger red).
pub const METER_HOT: Color32 = Color32::from_rgb(0xf5, 0xd0, 0x3c);
/// Unlit LED segments and rings: present but clearly off.
pub const LED_OFF: Color32 = Color32::from_rgb(0x25, 0x27, 0x2b);

// ---- Light

/// The 1px highlight along the top edge of a raised surface.
pub const EDGE_LIGHT: Color32 = Color32::from_rgba_premultiplied(41, 41, 41, 41);
/// The dark outline and bottom edge of a raised surface.
pub const EDGE_DARK: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 150);
/// How much lighter (0–1, towards white) a raised surface is at its top than its bottom.
pub const SHEEN: f32 = 0.08;
/// Shadow along the inner top edge of a recessed well.
pub const WELL_SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 110);
/// Faint light catching the bottom lip of a well.
pub const WELL_LIP: Color32 = Color32::from_rgba_premultiplied(9, 9, 9, 9);
/// The dark rim of a well.
pub const WELL_EDGE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 170);
/// The dimmed backdrop of full-window overlays (such as the file drop hint).
pub const OVERLAY: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 200);
/// The glass reflection over readout screens.
pub const GLASS: Color32 = Color32::from_rgba_premultiplied(7, 7, 7, 7);
/// A much gentler sheen for large panels, so they read as satin rather than buttons.
pub const PANEL_SHEEN: f32 = 0.025;

// ---- Shape

/// Corner radius of panels.
pub const R_PANEL: u8 = 4;
/// Corner radius of controls (buttons, pads, caps, screens, fields).
pub const R_CONTROL: u8 = 3;

// ---- Text

pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe9, 0xeb);
/// Labels and secondary text.
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xa9, 0xad, 0xb5);
/// Section headings and column headers: brighter than labels, softer than content.
pub const HEADING: Color32 = Color32::from_rgb(0xd3, 0xd6, 0xdb);

// ---- Meaning (unchanged hues)

pub const WARN: Color32 = Color32::from_rgb(0xf5, 0xa6, 0x3c);
pub const DANGER: Color32 = Color32::from_rgb(0xef, 0x5b, 0x5b);
pub const OK: Color32 = Color32::from_rgb(0x5f, 0xd0, 0x8a);

pub const DECK_A: Color32 = Color32::from_rgb(0x3f, 0xb6, 0xff);
pub const DECK_B: Color32 = Color32::from_rgb(0xff, 0x8a, 0x3d);

/// Waveform band colours: low (kick/bass), mid, high (hats).
/// Soft but clearly separated hues (blue, amber, mint), so lows, mids and highs are easy
/// to tell apart in the waveform without being glaring.
pub const BAND_LOW: Color32 = Color32::from_rgb(0x5c, 0x9c, 0xf2); // blue
pub const BAND_MID: Color32 = Color32::from_rgb(0xf0, 0xa8, 0x5c); // amber
pub const BAND_HIGH: Color32 = Color32::from_rgb(0x96, 0xe6, 0xc2); // mint

/// Musical key readouts.
pub const KEY: Color32 = Color32::from_rgb(0xb7, 0x8c, 0xff);

/// The app accent: selection and focus in areas shared by both decks (library, sidebar,
/// dialogs), never inside the decks or the mixer. A bluer violet than [`KEY`], which is
/// only ever text, so the two roles stay apart.
pub const ACCENT: Color32 = Color32::from_rgb(0x9c, 0x7e, 0xff);

/// The background of a selected row (library table, sidebar sets) and selected text: a
/// low-strength [`ACCENT`] tint on the window.
pub fn selected_fill() -> Color32 {
    crate::widgets::mix(BG, ACCENT, 0.2)
}

pub const PLAYHEAD: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const CUE_MARKER: Color32 = Color32::from_rgb(0xff, 0xd2, 0x3f);
/// Hot cue pads and markers.
pub const HOT_CUE: Color32 = Color32::from_rgb(0xff, 0x5f, 0xa2);
/// Beat loop pads and the active loop region.
pub const LOOP: Color32 = Color32::from_rgb(0x5f, 0xd0, 0x8a);

pub fn deck_colour(deck: DeckId) -> Color32 {
    match deck {
        DeckId::A => DECK_A,
        DeckId::B => DECK_B,
    }
}

/// The accent edge along the top of a deck's panels and lane: the deck colour muted
/// towards the panel, so full strength stays for its lit toggles, tag and title, and deck
/// B's frame doesn't read as the waveform mids or a warning.
pub fn deck_frame(deck: DeckId) -> Color32 {
    crate::widgets::mix(PANEL, deck_colour(deck), 0.7)
}

/// The fill of a control lit in `colour` (pads, switched-on toggles, primary buttons):
/// backlit in its colour over the raised surface, a little deeper while pressed.
pub fn lit_fill(colour: Color32, pressed: bool) -> Color32 {
    crate::widgets::mix(RAISED, colour, if pressed { 0.95 } else { 0.8 })
}

/// Labels and icons on a [`lit_fill`].
pub const LIT_INK: Color32 = BG;

/// Glyph scale for Inter, chosen so its rows are as tall as egui's default UI font (within
/// 1/32 pt, egui's rounding step) at every size we use, so swapping the font moves nothing.
pub const INTER_SCALE: f32 = 0.95;
/// Glyph scale for JetBrains Mono, matching the rows of egui's default monospace font.
pub const MONO_SCALE: f32 = 0.882;

/// The bundled fonts (see `assets/fonts/README.md`): Inter for the interface and JetBrains
/// Mono (every digit the same width) for readouts, with egui's own fonts kept as fallbacks
/// for symbols they lack.
pub fn fonts() -> egui::FontDefinitions {
    use egui::{FontData, FontFamily, FontTweak};
    use std::sync::Arc;
    let mut fonts = egui::FontDefinitions::default();
    let tweaked = |bytes: &'static [u8], scale: f32| {
        Arc::new(FontData::from_static(bytes).tweak(FontTweak {
            scale,
            ..Default::default()
        }))
    };
    fonts.font_data.insert(
        "Inter".into(),
        tweaked(
            include_bytes!("../assets/fonts/Inter-Regular.ttf"),
            INTER_SCALE,
        ),
    );
    fonts.font_data.insert(
        "JetBrainsMono".into(),
        tweaked(
            include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
            MONO_SCALE,
        ),
    );
    for (family, name) in [
        (FontFamily::Proportional, "Inter"),
        (FontFamily::Monospace, "JetBrainsMono"),
    ] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.into());
    }
    // Inter and egui's fonts have no ♭/♯ (key names like "A♭ major"); JetBrains Mono does,
    // so it is the UI family's fallback for them.
    if let Some(ui) = fonts.families.get_mut(&FontFamily::Proportional) {
        ui.insert(2, "JetBrainsMono".into());
    }
    fonts
}

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(1.0, HAIRLINE);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.window_corner_radius = CornerRadius::same(R_PANEL);
    v.menu_corner_radius = CornerRadius::same(R_CONTROL);
    // Text fields and scroll tracks read as recessed wells.
    v.extreme_bg_color = WELL;
    v.text_edit_bg_color = Some(WELL);
    v.faint_bg_color = Color32::from_rgb(0x13, 0x14, 0x17);
    v.code_bg_color = WELL;
    v.override_text_color = Some(TEXT);
    // Selection and focus outside the decks use the app accent: a tinted row or text
    // selection, and an accent outline on focused fields.
    v.selection.bg_fill = selected_fill();
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = HEADING;
    let radius = CornerRadius::same(R_CONTROL);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, HAIRLINE);
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, EDGE_DARK);
    v.widgets.hovered.bg_fill = RAISED_HOVER;
    v.widgets.hovered.weak_bg_fill = RAISED_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x56, 0x5a, 0x62));
    v.widgets.active.bg_fill = Color32::from_rgb(0x25, 0x27, 0x2b);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x25, 0x27, 0x2b);
    v.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x68, 0x6c, 0x75));
    v.widgets.open.bg_fill = RAISED_HOVER;
    v.widgets.open.weak_bg_fill = RAISED_HOVER;
    v.widgets.open.bg_stroke = Stroke::new(1.0, HAIRLINE);
    ctx.set_visuals(v);
    ctx.set_fonts(fonts());

    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.slider_width = 140.0;
    });
}

/// WCAG 2 contrast ratio between two opaque colours (1 to 21). Used by the tests that keep
/// the palette readable.
#[cfg(test)]
pub fn contrast(a: Color32, b: Color32) -> f32 {
    fn channel(c: u8) -> f32 {
        let c = f32::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let lum =
        |c: Color32| 0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b());
    let (la, lb) = (lum(a), lum(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACES: [(&str, Color32); 5] = [
        ("WELL", WELL),
        ("BG", BG),
        ("PANEL", PANEL),
        ("RAISED", RAISED),
        ("RAISED_HOVER", RAISED_HOVER),
    ];

    #[test]
    fn primary_text_has_at_least_7_to_1_contrast() {
        for (name, s) in SURFACES {
            for (t, c) in [("TEXT", TEXT), ("HEADING", HEADING)] {
                let r = contrast(c, s);
                assert!(r >= 7.0, "{t} on {name}: {r:.2}:1");
            }
        }
    }

    #[test]
    fn labels_have_at_least_4_5_to_1_contrast() {
        for (name, s) in SURFACES {
            let r = contrast(TEXT_DIM, s);
            assert!(r >= 4.5, "TEXT_DIM on {name}: {r:.2}:1");
        }
    }

    #[test]
    fn the_check_catches_a_too_dark_label() {
        let too_dark = Color32::from_rgb(0x5a, 0x5d, 0x63);
        assert!(contrast(too_dark, RAISED) < 4.5);
    }

    /// Minimum contrast between surfaces that sit on each other, so panels stand out from
    /// the window and controls and wells from their panel.
    const MIN_STEP: f32 = 1.2;

    /// (name, surface, the surface it sits on).
    fn steps(
        well: Color32,
        bg: Color32,
        panel: Color32,
        raised: Color32,
    ) -> [(&'static str, f32); 3] {
        [
            ("PANEL on BG", contrast(panel, bg)),
            ("WELL in PANEL", contrast(well, panel)),
            ("RAISED on PANEL", contrast(raised, panel)),
        ]
    }

    #[test]
    fn surface_steps_are_clearly_visible() {
        for (name, r) in steps(WELL, BG, PANEL, RAISED) {
            assert!(r >= MIN_STEP, "{name}: {r:.2}:1");
        }
    }

    #[test]
    fn the_step_check_catches_the_old_flat_palette() {
        // The studio satin values before this change: every step about 1.1:1.
        let old = steps(
            Color32::from_rgb(0x08, 0x09, 0x0a),
            Color32::from_rgb(0x0d, 0x0e, 0x10),
            Color32::from_rgb(0x17, 0x18, 0x1b),
            Color32::from_rgb(0x22, 0x24, 0x28),
        );
        assert!(old.iter().all(|&(_, r)| r < MIN_STEP), "{old:?}");
    }

    /// Every colour a control can be lit in.
    const LIT: [(&str, Color32); 6] = [
        ("DECK_A", DECK_A),
        ("ACCENT", ACCENT),
        ("DECK_B", DECK_B),
        ("HOT_CUE", HOT_CUE),
        ("LOOP", LOOP),
        ("OK", OK),
    ];

    #[test]
    fn text_on_a_selected_row_stays_readable() {
        let row = selected_fill();
        for (name, c, min) in [
            ("TEXT", TEXT, 7.0),
            ("TEXT_DIM", TEXT_DIM, 4.5),
            ("KEY", KEY, 4.5),
        ] {
            let r = contrast(c, row);
            assert!(r >= min, "{name} on the selected row: {r:.2}:1");
        }
    }

    #[test]
    fn selected_text_in_a_field_stays_readable() {
        // egui inks selected text in the selection stroke colour, on the selection fill.
        let r = contrast(ACCENT, selected_fill());
        assert!(r >= 4.5, "ACCENT on the selected fill: {r:.2}:1");
    }

    #[test]
    fn labels_on_lit_controls_have_at_least_4_5_to_1_contrast() {
        for (name, c) in LIT {
            for pressed in [false, true] {
                let r = contrast(LIT_INK, lit_fill(c, pressed));
                assert!(r >= 4.5, "LIT_INK on {name} (pressed {pressed}): {r:.2}:1");
            }
        }
    }

    /// Sizes the interface uses (including egui's own text styles).
    const SIZES: [f32; 14] = [
        8.0, 9.0, 10.0, 11.0, 12.0, 12.5, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0, 22.0, 24.0,
    ];

    fn row_heights(defs: egui::FontDefinitions, family: egui::FontFamily) -> Vec<f32> {
        let ctx = egui::Context::default();
        ctx.set_fonts(defs);
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        ctx.fonts_mut(|f| {
            SIZES
                .iter()
                .map(|&s| f.row_height(&egui::FontId::new(s, family.clone())))
                .collect()
        })
    }

    #[test]
    fn bundled_fonts_keep_row_heights() {
        // Swapping fonts must not move anything: rows stay as tall as with egui's default
        // fonts at every size, within 1/32 pt (egui rounds metrics to that step, so no
        // single scale can match every size exactly).
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let old = row_heights(egui::FontDefinitions::default(), family.clone());
            let new = row_heights(fonts(), family.clone());
            for ((size, a), b) in SIZES.iter().zip(&old).zip(&new) {
                assert!(
                    (a - b).abs() <= 1.0 / 32.0,
                    "{family:?} {size}pt: {a} vs {b}"
                );
            }
        }
    }

    #[test]
    #[ignore = "probe for tuning INTER_SCALE / MONO_SCALE"]
    fn probe_font_scales() {
        use egui::{FontData, FontFamily, FontTweak};
        for (family, name, bytes) in [
            (
                FontFamily::Proportional,
                "Inter",
                &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..],
            ),
            (
                FontFamily::Monospace,
                "JetBrainsMono",
                &include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")[..],
            ),
        ] {
            let old = row_heights(egui::FontDefinitions::default(), family.clone());
            let centre = if name == "Inter" { 0.95 } else { 0.88 };
            for k in -40..=40 {
                let scale = centre + k as f32 * 0.0005;
                let mut defs = egui::FontDefinitions::default();
                let bytes: &'static [u8] = Box::leak(bytes.to_vec().into_boxed_slice());
                defs.font_data.insert(
                    name.into(),
                    std::sync::Arc::new(FontData::from_static(bytes).tweak(FontTweak {
                        scale,
                        ..Default::default()
                    })),
                );
                defs.families
                    .get_mut(&family)
                    .unwrap()
                    .insert(0, name.into());
                let new = row_heights(defs, family.clone());
                let off = old
                    .iter()
                    .zip(&new)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0f32, f32::max);
                if off <= 0.2 {
                    println!("{name} scale {scale:.4}: max row diff {off:.4}pt");
                }
            }
            println!("{name} old {old:?}");
        }
    }
}
