//! djkitty's dark theme: calm surfaces, one accent colour per deck, readable band colours.

use egui::{Color32, CornerRadius, Stroke, Visuals};

use djk_core::DeckId;

pub const BG: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x19, 0x1c, 0x22);
pub const RAISED: Color32 = Color32::from_rgb(0x23, 0x27, 0x2f);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe8, 0xec);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8b, 0x92, 0x9e);
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

pub const PLAYHEAD: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const CUE_MARKER: Color32 = Color32::from_rgb(0xff, 0xd2, 0x3f);

pub fn deck_colour(deck: DeckId) -> Color32 {
    match deck {
        DeckId::A => DECK_A,
        DeckId::B => DECK_B,
    }
}

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = Color32::from_rgb(0x1d, 0x20, 0x27);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = Color32::from_rgb(0x2b, 0x4f, 0x7a);
    v.selection.stroke = Stroke::new(1.0, DECK_A);
    v.hyperlink_color = DECK_A;
    let radius = CornerRadius::same(6);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x30, 0x34, 0x3d));
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x2d, 0x32, 0x3b);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2d, 0x32, 0x3b);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x4a, 0x50, 0x5c));
    v.widgets.active.bg_fill = Color32::from_rgb(0x1a, 0x1d, 0x22);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x1a, 0x1d, 0x22);
    v.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x5a, 0x61, 0x6e));
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x2a, 0x2e, 0x37));
    v.window_corner_radius = CornerRadius::same(10);
    ctx.set_visuals(v);

    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.slider_width = 140.0;
    });
}
