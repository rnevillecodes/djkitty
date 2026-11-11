//! Custom-painted DJ controls: faders, knobs, transport buttons and level
//! meters. They borrow from hardware (fader caps, lit button rings, segmented meters) so
//! the screen reads like the controller in front of you.
//!
//! Every value control follows the same rules: drag to change (hold Shift for fine
//! control), and Ctrl/Cmd+click or double-click to reset. A plain click never moves them.

use egui::epaint::PathShape;
use egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::theme;

/// Linear blend between two colours (`t` = 0 gives `a`).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// Fill `rect` with a vertical gradient from `top` to `bottom`.
pub fn vertical_gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    let i = mesh.vertices.len() as u32;
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
    painter.add(Shape::mesh(mesh));
}

/// A soft glow: concentric circles fading out from `radius`.
fn glow(painter: &Painter, centre: Pos2, radius: f32, colour: Color32, strength: f32) {
    for k in 1..=6 {
        let a = strength * (1.0 - k as f32 / 7.0) * 0.35;
        painter.circle_stroke(
            centre,
            radius + k as f32 * 1.6,
            Stroke::new(1.8, colour.gamma_multiply(a)),
        );
    }
}

/// Shared value-control input handling. Returns the new normalised value, if any.
///
/// `travel` converts pointer movement along `axis` to value. Only drags move the value:
/// a plain click does nothing, so a double-click resets without moving first.
fn value_input(
    ui: &Ui,
    resp: &Response,
    t: f32,
    default_t: f32,
    travel: f32,
    axis: Vec2,
) -> Option<f32> {
    let reset = ui.input(|i| i.modifiers.command);
    if resp.double_clicked() || (resp.clicked() && reset) {
        return Some(default_t);
    }
    if resp.dragged() && !reset {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let d = resp.drag_delta().dot(axis) / travel.max(1.0) * fine;
        if d != 0.0 {
            return Some((t + d).clamp(0.0, 1.0));
        }
    }
    None
}

/// Visual options for [`fader`].
#[derive(Clone, Copy)]
pub struct FaderStyle {
    pub accent: Color32,
    /// Number of tick marks along the track (0 for none).
    pub ticks: usize,
    /// Mark and gently snap to the centre (pitch, crossfader).
    pub centre_detent: bool,
    /// Light the track from the low end up to the cap (channel faders).
    pub fill: bool,
}

/// A hardware-style fader. `t` is the normalised value (0–1); for vertical faders 1 is
/// at the top, unless `invert` (pitch faders, where pulling down speeds up, as on CDJs).
///
/// Returns the response and the new value, if changed.
pub fn fader(
    ui: &mut Ui,
    size: Vec2,
    vertical: bool,
    invert: bool,
    t: f32,
    default_t: f32,
    style: FaderStyle,
) -> (Response, Option<f32>) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let cap = if vertical {
        vec2(size.x.min(34.0), 18.0)
    } else {
        vec2(18.0, size.y.min(30.0))
    };
    let (start, end) = if vertical {
        let top = pos2(rect.center().x, rect.top() + cap.y / 2.0);
        let bottom = pos2(rect.center().x, rect.bottom() - cap.y / 2.0);
        if invert { (top, bottom) } else { (bottom, top) }
    } else {
        let l = pos2(rect.left() + cap.x / 2.0, rect.center().y);
        let r = pos2(rect.right() - cap.x / 2.0, rect.center().y);
        if invert { (r, l) } else { (l, r) }
    };
    let span = end - start;
    let travel = span.length();
    let axis = span / travel.max(1.0);
    let mut out = value_input(ui, &resp, t, default_t, travel, axis);
    if style.centre_detent
        && let Some(v) = out.as_mut()
        && resp.dragged()
        && (*v - 0.5).abs() < 0.012
    {
        *v = 0.5;
    }
    let shown = out.unwrap_or(t);

    let painter = ui.painter_at(rect.expand(8.0));
    // Ticks either side of the slot.
    if style.ticks > 1 {
        for k in 0..style.ticks {
            let f = k as f32 / (style.ticks - 1) as f32;
            let p = start + span * f;
            let major =
                k == 0 || k == style.ticks - 1 || (style.centre_detent && (f - 0.5).abs() < 1e-3);
            let len = if major { 7.0 } else { 4.0 };
            let col = if major {
                theme::TEXT_DIM
            } else {
                theme::TEXT_DIM.gamma_multiply(0.5)
            };
            let n = vec2(axis.y, -axis.x); // normal
            let off = if vertical {
                size.x / 2.0 - 1.0
            } else {
                size.y / 2.0 - 1.0
            };
            for side in [-1.0, 1.0] {
                let a = p + n * side * off;
                let b = a - n * side * len;
                painter.line_segment([a, b], Stroke::new(1.0, col));
            }
        }
    }
    // Slot.
    let slot_w = 5.0;
    let slot = Rect::from_two_pos(start, end).expand2(if vertical {
        vec2(slot_w / 2.0, 2.0)
    } else {
        vec2(2.0, slot_w / 2.0)
    });
    painter.rect_filled(
        slot,
        CornerRadius::same(3),
        Color32::from_rgb(0x07, 0x08, 0x0a),
    );
    painter.rect_stroke(
        slot,
        CornerRadius::same(3),
        Stroke::new(1.0, Color32::from_white_alpha(10)),
        StrokeKind::Outside,
    );
    let cap_centre = start + span * shown;
    if style.fill {
        let lit = Rect::from_two_pos(start, cap_centre).expand2(if vertical {
            vec2(1.0, 0.0)
        } else {
            vec2(0.0, 1.0)
        });
        painter.rect_filled(
            lit,
            CornerRadius::same(2),
            style.accent.gamma_multiply(0.75),
        );
    }
    if style.centre_detent {
        let c = start + span * 0.5;
        let n = vec2(axis.y, -axis.x);
        painter.line_segment(
            [c - n * (slot_w + 3.0), c + n * (slot_w + 3.0)],
            Stroke::new(1.5, style.accent.gamma_multiply(0.8)),
        );
    }
    // Cap: drop shadow, two-tone body, grip lines and an indicator line.
    let cap_rect = Rect::from_center_size(cap_centre, cap);
    let hot = resp.hovered() || resp.dragged();
    painter.rect_filled(
        cap_rect.translate(vec2(0.0, 2.0)).expand(1.0),
        CornerRadius::same(4),
        Color32::from_black_alpha(120),
    );
    let (top, bottom) = if hot {
        (
            Color32::from_rgb(0x5a, 0x60, 0x6c),
            Color32::from_rgb(0x34, 0x38, 0x41),
        )
    } else {
        (
            Color32::from_rgb(0x4a, 0x4f, 0x5a),
            Color32::from_rgb(0x2a, 0x2d, 0x34),
        )
    };
    painter.rect_filled(cap_rect, CornerRadius::same(4), bottom);
    vertical_gradient(&painter, cap_rect.shrink(1.5), top, bottom);
    painter.rect_stroke(
        cap_rect,
        CornerRadius::same(4),
        Stroke::new(1.0, Color32::from_black_alpha(160)),
        StrokeKind::Outside,
    );
    let n = vec2(axis.y, -axis.x);
    let half = if vertical {
        cap.x / 2.0 - 4.0
    } else {
        cap.y / 2.0 - 4.0
    };
    for k in [-1.0f32, 1.0] {
        let c = cap_centre + axis * k * 4.5;
        painter.line_segment(
            [c - n * half, c + n * half],
            Stroke::new(1.0, Color32::from_black_alpha(90)),
        );
    }
    painter.line_segment(
        [cap_centre - n * half, cap_centre + n * half],
        Stroke::new(2.0, if hot { Color32::WHITE } else { style.accent }),
    );
    if hot {
        ui.ctx().set_cursor_icon(if vertical {
            egui::CursorIcon::ResizeVertical
        } else {
            egui::CursorIcon::ResizeHorizontal
        });
    }
    (resp, out)
}

const KNOB_START: f32 = std::f32::consts::PI * 0.75; // 7:30 o'clock
const KNOB_SWEEP: f32 = std::f32::consts::PI * 1.5; // to 4:30

/// A rotary knob (drag up/down). `t` is normalised 0–1. Returns the new value, if changed.
///
/// A `bipolar` knob (like an EQ) has its neutral point at 12 o'clock: the arc grows from
/// there in either direction and dragging snaps gently to the centre.
pub fn knob(
    ui: &mut Ui,
    diameter: f32,
    t: f32,
    default_t: f32,
    accent: Color32,
    enabled: bool,
    bipolar: bool,
) -> (Response, Option<f32>) {
    let sense = if enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(diameter), sense);
    let out = if enabled {
        let v = value_input(ui, &resp, t, default_t, 140.0, vec2(0.0, -1.0));
        if bipolar && resp.dragged() {
            v.map(|v| if (v - 0.5).abs() < 0.015 { 0.5 } else { v })
        } else {
            v
        }
    } else {
        None
    };
    let shown = out.unwrap_or(t);
    let painter = ui.painter_at(rect.expand(4.0));
    let c = rect.center();
    let r = diameter / 2.0;
    let arc = |from: f32, to: f32, radius: f32, stroke: Stroke| {
        let steps = 40;
        let pts: Vec<Pos2> = (0..=steps)
            .map(|k| {
                let a = from + (to - from) * k as f32 / steps as f32;
                c + vec2(a.cos(), a.sin()) * radius
            })
            .collect();
        painter.add(Shape::Path(PathShape::line(pts, stroke)));
    };
    let accent = if enabled {
        accent
    } else {
        theme::TEXT_DIM.gamma_multiply(0.5)
    };
    arc(
        KNOB_START,
        KNOB_START + KNOB_SWEEP,
        r - 2.0,
        Stroke::new(3.0, Color32::from_rgb(0x07, 0x08, 0x0a)),
    );
    let from = if bipolar { 0.5 } else { 0.0 };
    if (shown - from).abs() > 0.001 {
        arc(
            KNOB_START + KNOB_SWEEP * from,
            KNOB_START + KNOB_SWEEP * shown,
            r - 2.0,
            Stroke::new(3.0, accent),
        );
    }
    if bipolar {
        // Centre mark at 12 o'clock.
        let up = vec2(0.0, -1.0);
        painter.line_segment(
            [c + up * (r - 4.0), c + up * r],
            Stroke::new(1.5, theme::TEXT_DIM),
        );
    }
    // Body with a highlight towards the top.
    let body_r = r - 7.0;
    painter.circle_filled(
        c + vec2(0.0, 1.5),
        body_r + 1.0,
        Color32::from_black_alpha(140),
    );
    painter.circle_filled(c, body_r, Color32::from_rgb(0x2a, 0x2d, 0x34));
    painter.circle_filled(
        c - vec2(0.0, body_r * 0.25),
        body_r * 0.7,
        Color32::from_rgb(0x34, 0x38, 0x41),
    );
    painter.circle_stroke(c, body_r, Stroke::new(1.0, Color32::from_white_alpha(18)));
    let a = KNOB_START + KNOB_SWEEP * shown;
    let dir = vec2(a.cos(), a.sin());
    painter.line_segment(
        [c + dir * body_r * 0.25, c + dir * (body_r - 2.0)],
        Stroke::new(
            2.0,
            if resp.hovered() || resp.dragged() {
                Color32::WHITE
            } else {
                theme::TEXT
            },
        ),
    );
    if enabled && (resp.hovered() || resp.dragged()) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    (resp, out)
}

/// What a round transport button draws in its centre.
#[derive(Clone, Copy)]
pub enum TransportIcon {
    /// Play/pause: a triangle and two bars, as printed on CDJs and controllers.
    PlayPause,
    Cue,
}

/// A round, backlit transport button. `led` is the ring colour and `lit` its brightness
/// (0 = off, 1 = fully lit). Senses click and drag so press-and-hold works (cue preview).
pub fn transport_button(
    ui: &mut Ui,
    diameter: f32,
    icon: TransportIcon,
    led: Color32,
    lit: f32,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(diameter + 8.0), sense);
    let painter = ui.painter_at(rect.expand(10.0));
    let c = rect.center();
    let r = diameter / 2.0;
    let pressed = resp.is_pointer_button_down_on();
    let hovered = resp.hovered() && enabled;
    if lit > 0.05 {
        glow(&painter, c, r, led, lit);
    }
    // Ring (the LED) and body.
    painter.circle_filled(c + vec2(0.0, 2.0), r + 1.0, Color32::from_black_alpha(150));
    let ring = mix(Color32::from_rgb(0x3a, 0x3e, 0x47), led, lit);
    painter.circle_filled(c, r, ring);
    let body_r = r - 3.5;
    let (top, bottom) = match (pressed, hovered) {
        (true, _) => (
            Color32::from_rgb(0x1c, 0x1e, 0x23),
            Color32::from_rgb(0x26, 0x29, 0x30),
        ),
        (false, true) => (
            Color32::from_rgb(0x3c, 0x41, 0x4b),
            Color32::from_rgb(0x24, 0x27, 0x2e),
        ),
        _ => (
            Color32::from_rgb(0x32, 0x36, 0x3e),
            Color32::from_rgb(0x1e, 0x21, 0x27),
        ),
    };
    painter.circle_filled(c, body_r, bottom);
    painter.circle_filled(
        c - vec2(0.0, body_r * 0.22),
        body_r * 0.78,
        mix(bottom, top, 0.7),
    );
    painter.circle_stroke(c, body_r, Stroke::new(1.0, Color32::from_black_alpha(160)));
    let ink = if !enabled {
        theme::TEXT_DIM.gamma_multiply(0.5)
    } else if lit > 0.5 {
        mix(theme::TEXT, led, 0.35)
    } else {
        theme::TEXT
    };
    let c = c + if pressed { vec2(0.0, 1.0) } else { Vec2::ZERO };
    match icon {
        TransportIcon::PlayPause => {
            let s = diameter * 0.16;
            let tri = vec![
                c + vec2(-s * 1.55, -s),
                c + vec2(-s * 1.55, s),
                c + vec2(-s * 0.05, 0.0),
            ];
            painter.add(Shape::convex_polygon(tri, ink, Stroke::NONE));
            for x in [0.45, 1.15] {
                let bar = Rect::from_min_size(c + vec2(s * x, -s), vec2(s * 0.42, s * 2.0));
                painter.rect_filled(bar, CornerRadius::same(1), ink);
            }
        }
        TransportIcon::Cue => {
            painter.text(
                c,
                Align2::CENTER_CENTER,
                "CUE",
                FontId::proportional(diameter * 0.26),
                ink,
            );
        }
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Meter ballistics: instant attack, ~20 dB/s fall, and a peak hold that waits 1 s.
#[derive(Clone, Copy, Default)]
pub struct Ballistic {
    /// Displayed level, 0–1 on the meter scale.
    pub level: f32,
    pub hold: f32,
    hold_age: f32,
}

/// Bottom of the meter scale, in dBFS.
const METER_FLOOR_DB: f32 = -48.0;

/// Map a linear peak to the 0–1 meter scale.
pub fn meter_scale(peak: f32) -> f32 {
    if peak <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * peak.log10();
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

impl Ballistic {
    pub fn update(&mut self, peak: f32, dt: f32) {
        let target = meter_scale(peak);
        let fall = 20.0 / -METER_FLOOR_DB * dt;
        self.level = target.max(self.level - fall);
        if target >= self.hold {
            self.hold = target;
            self.hold_age = 0.0;
        } else {
            self.hold_age += dt;
            if self.hold_age > 1.0 {
                self.hold = (self.hold - fall * 1.5).max(self.level);
            }
        }
    }

    /// True while the meter is still moving (so the UI keeps repainting).
    pub fn active(&self) -> bool {
        self.level > 0.0 || self.hold > 0.0
    }
}

/// A vertical segmented LED meter in `rect`.
pub fn meter(painter: &Painter, rect: Rect, b: Ballistic) {
    const SEGMENTS: usize = 18;
    painter.rect_filled(
        rect.expand(1.5),
        CornerRadius::same(2),
        Color32::from_rgb(0x07, 0x08, 0x0a),
    );
    let gap = 1.5;
    let seg_h = (rect.height() - gap * (SEGMENTS - 1) as f32) / SEGMENTS as f32;
    let hold_seg = (b.hold * SEGMENTS as f32).ceil() as usize;
    for k in 0..SEGMENTS {
        let f = (k as f32 + 1.0) / SEGMENTS as f32;
        let colour = if f > 0.94 {
            theme::DANGER
        } else if f > 0.8 {
            Color32::from_rgb(0xf5, 0xd0, 0x3c)
        } else {
            theme::OK
        };
        let lit = b.level >= f - 0.5 / SEGMENTS as f32 || (k + 1 == hold_seg && b.hold > 0.02);
        let y1 = rect.bottom() - k as f32 * (seg_h + gap);
        let seg = Rect::from_min_max(pos2(rect.left(), y1 - seg_h), pos2(rect.right(), y1));
        painter.rect_filled(
            seg,
            CornerRadius::same(1),
            if lit {
                colour
            } else {
                colour.gamma_multiply(0.12)
            },
        );
    }
}

/// Paint a recessed "screen" (for BPM, key and time readouts) and return its inner rect.
pub fn screen(painter: &Painter, rect: Rect) -> Rect {
    painter.rect_filled(
        rect,
        CornerRadius::same(6),
        Color32::from_rgb(0x0b, 0x0d, 0x10),
    );
    // Inner shadow along the top edge.
    vertical_gradient(
        painter,
        Rect::from_min_size(rect.min + vec2(3.0, 1.0), vec2(rect.width() - 6.0, 6.0)),
        Color32::from_black_alpha(90),
        Color32::TRANSPARENT,
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, Color32::from_white_alpha(12)),
        StrokeKind::Inside,
    );
    rect.shrink(8.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_scale_maps_dbfs() {
        assert_eq!(meter_scale(0.0), 0.0);
        assert!((meter_scale(1.0) - 1.0).abs() < 1e-6);
        // -24 dBFS sits half way up a -48 dB scale.
        assert!((meter_scale(10f32.powf(-24.0 / 20.0)) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn ballistics_attack_instantly_and_fall_slowly() {
        let mut b = Ballistic::default();
        b.update(1.0, 0.016);
        assert!((b.level - 1.0).abs() < 1e-6);
        b.update(0.0, 0.1);
        // 20 dB/s for 0.1 s is 2 dB, i.e. 2/48 of the scale.
        assert!((b.level - (1.0 - 2.0 / 48.0)).abs() < 1e-4, "{}", b.level);
        assert!((b.hold - 1.0).abs() < 1e-6, "hold stays for a second");
    }

    #[test]
    fn mix_blends_colours() {
        let c = mix(Color32::BLACK, Color32::WHITE, 0.5);
        assert!((127..=128).contains(&c.r()));
    }
}

// ---------------------------------------------------------------- inputs and icons

/// Height of single-line text fields, so they line up with buttons.
pub const FIELD_HEIGHT: f32 = 28.0;

/// Space between a text field's edge and its text.
pub const FIELD_TEXT_INSET: f32 = 8.0;

/// A single-line text field with its text vertically centred.
pub fn text_field(text: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .align(Align2::LEFT_CENTER)
        .min_size(vec2(0.0, FIELD_HEIGHT))
        .margin(vec2(FIELD_TEXT_INSET, 0.0))
}

/// SVG icons, drawn white so they can be tinted.
pub mod icons {
    use egui::ImageSource;

    pub const PLUS: ImageSource<'static> = egui::include_image!("../assets/icons/plus.svg");
    pub const CHEVRON_DOWN: ImageSource<'static> =
        egui::include_image!("../assets/icons/chevron-down.svg");
    pub const CHEVRON_UP: ImageSource<'static> =
        egui::include_image!("../assets/icons/chevron-up.svg");
    pub const FILE: ImageSource<'static> = egui::include_image!("../assets/icons/file.svg");
    pub const FOLDER: ImageSource<'static> = egui::include_image!("../assets/icons/folder.svg");
}

/// An icon image at `size` points, tinted `tint`.
pub fn icon(source: egui::ImageSource<'static>, size: f32, tint: Color32) -> egui::Image<'static> {
    egui::Image::new(source)
        .fit_to_exact_size(vec2(size, size))
        .tint(tint)
}
