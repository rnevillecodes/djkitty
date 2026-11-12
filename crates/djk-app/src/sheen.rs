//! "Studio satin" lighting: the few painting primitives every surface uses, so the whole
//! app is lit the same way, from above.
//!
//! - [`raised`]: a surface standing proud (panel, button, pad, cap): lighter at the top,
//!   a 1px highlight along its top edge and a dark outline.
//! - [`well`]: a recessed area (screen, waveform lane, fader slot, meter): darker than
//!   its surroundings with a shadow along its inner top edge.
//! - [`glass`]: a faint reflection across the top of a readout screen.
//! - [`round_lit`]: a round control (knob cap, transport button) lit like [`raised`].
//!
//! Gradients are meshes (egui has no gradient fills), which egui doesn't anti-alias, so
//! each body is finished with an anti-aliased outline that hides the edge.

use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2};

use crate::theme;

/// Blend `c` towards white by `t` (0–1), keeping its alpha.
pub fn lighten(c: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8| (f32::from(x) + (255.0 - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(c.r()), l(c.g()), l(c.b()), c.a())
}

/// Blend `c` towards black by `t` (0–1), keeping its alpha.
pub fn darken(c: Color32, t: f32) -> Color32 {
    let t = 1.0 - t.clamp(0.0, 1.0);
    let l = |x: u8| (f32::from(x) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(c.r()), l(c.g()), l(c.b()), c.a())
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    crate::widgets::mix(a, b, t)
}

/// Outline of a rounded rectangle, clockwise from the top-left corner's top end.
fn rounded_outline(rect: Rect, radius: f32) -> Vec<Pos2> {
    let r = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    if r < 0.5 {
        return vec![
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ];
    }
    const STEPS: usize = 4;
    let corners = [
        // (centre, start angle): top-right, bottom-right, bottom-left, top-left.
        (pos2(rect.right() - r, rect.top() + r), -90.0f32),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 90.0),
        (pos2(rect.left() + r, rect.top() + r), 180.0),
    ];
    let mut pts = Vec::with_capacity(4 * (STEPS + 1));
    for (c, start) in corners {
        for k in 0..=STEPS {
            let a = (start + 90.0 * k as f32 / STEPS as f32).to_radians();
            pts.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    }
    pts
}

/// A mesh filling `outline` (convex, around `centre`) with a vertical gradient from `top`
/// at `rect.top()` to `bottom` at `rect.bottom()`.
fn gradient_fan(outline: &[Pos2], centre: Pos2, rect: Rect, top: Color32, bottom: Color32) -> Mesh {
    let colour_at = |y: f32| lerp(top, bottom, (y - rect.top()) / rect.height().max(1.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(centre, colour_at(centre.y));
    for &p in outline {
        mesh.colored_vertex(p, colour_at(p.y));
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    mesh
}

/// A rounded rect with a vertical gradient (used by [`raised`]).
pub fn gradient_rect(rect: Rect, radius: f32, top: Color32, bottom: Color32) -> Mesh {
    gradient_fan(
        &rounded_outline(rect, radius),
        rect.center(),
        rect,
        top,
        bottom,
    )
}

/// A raised surface lit from above. `sheen` is how much lighter its top is (see
/// [`theme::SHEEN`] and [`theme::PANEL_SHEEN`]).
pub fn raised(painter: &Painter, rect: Rect, radius: f32, base: Color32, sheen: f32) {
    painter.extend(raised_shapes(rect, radius, base, sheen, theme::EDGE_DARK));
}

/// The shapes of [`raised`] with a custom outline colour, for painting into a placeholder
/// behind content laid out afterwards (see `ui::card`).
pub fn raised_shapes(
    rect: Rect,
    radius: f32,
    base: Color32,
    sheen: f32,
    outline: Color32,
) -> Vec<Shape> {
    let r = radius.min(rect.width() / 2.0);
    let y = rect.top() + 0.5;
    vec![
        Shape::mesh(gradient_rect(rect, radius, lighten(base, sheen), base)),
        Shape::line_segment(
            [pos2(rect.left() + r, y), pos2(rect.right() - r, y)],
            Stroke::new(1.0, theme::EDGE_LIGHT),
        ),
        Shape::rect_stroke(rect, radius, Stroke::new(1.0, outline), StrokeKind::Inside),
    ]
}

/// A recessed well: darker than its surroundings, shadowed along its inner top edge, with
/// a faint lip of light along the bottom.
pub fn well(painter: &Painter, rect: Rect, radius: f32) {
    well_filled(painter, rect, radius, theme::WELL);
}

/// [`well`] with a custom fill (for wells that need to be a touch lighter or tinted).
pub fn well_filled(painter: &Painter, rect: Rect, radius: f32, fill: Color32) {
    painter.rect_filled(rect, radius, fill);
    let depth = (rect.height() * 0.5).min(6.0);
    let shadow = Rect::from_min_max(
        pos2(rect.left() + 1.0, rect.top() + 1.0),
        pos2(rect.right() - 1.0, rect.top() + 1.0 + depth),
    );
    if shadow.width() > 0.0 && shadow.height() > 0.0 {
        crate::widgets::vertical_gradient(
            painter,
            shadow,
            theme::WELL_SHADOW,
            Color32::TRANSPARENT,
        );
    }
    let r = radius.min(rect.width() / 2.0);
    let y = rect.bottom() - 0.5;
    painter.line_segment(
        [pos2(rect.left() + r, y), pos2(rect.right() - r, y)],
        Stroke::new(1.0, theme::WELL_LIP),
    );
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, theme::WELL_EDGE),
        StrokeKind::Inside,
    );
}

/// A faint glass reflection over the top of a readout screen, cut by a gentle diagonal.
/// Paint it after the screen's background and before its text.
pub fn glass(painter: &Painter, rect: Rect) {
    let r = rect.shrink(1.5);
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return;
    }
    let tint = theme::GLASS;
    painter.add(Shape::convex_polygon(
        vec![
            r.left_top(),
            r.right_top(),
            pos2(r.right(), r.top() + r.height() * 0.30),
            pos2(r.left(), r.top() + r.height() * 0.55),
        ],
        tint,
        Stroke::NONE,
    ));
}

/// A round control lit from above: a vertical gradient body, a highlight along its top
/// rim and a dark outline.
pub fn round_lit(painter: &Painter, centre: Pos2, radius: f32, base: Color32, sheen: f32) {
    const STEPS: usize = 40;
    let outline: Vec<Pos2> = (0..STEPS)
        .map(|k| {
            let a = std::f32::consts::TAU * k as f32 / STEPS as f32;
            pos2(centre.x + radius * a.cos(), centre.y + radius * a.sin())
        })
        .collect();
    let rect = Rect::from_center_size(centre, egui::Vec2::splat(2.0 * radius));
    painter.add(Shape::mesh(gradient_fan(
        &outline,
        centre,
        rect,
        lighten(base, sheen),
        darken(base, sheen * 0.6),
    )));
    // Highlight along the top third of the rim.
    let rim: Vec<Pos2> = (0..=16)
        .map(|k| {
            let a = (210.0 + 120.0 * k as f32 / 16.0f32).to_radians();
            pos2(
                centre.x + (radius - 0.75) * a.cos(),
                centre.y + (radius - 0.75) * a.sin(),
            )
        })
        .collect();
    painter.add(Shape::line(rim, Stroke::new(1.0, theme::EDGE_LIGHT)));
    painter.circle_stroke(centre, radius, Stroke::new(1.0, theme::EDGE_DARK));
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    fn luma(c: Color32) -> u32 {
        u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b())
    }

    #[test]
    fn gradient_rect_is_lighter_at_the_top_and_stays_inside() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(80.0, 30.0));
        let mesh = gradient_rect(rect, 3.0, lighten(theme::RAISED, 0.2), theme::RAISED);
        let top = mesh
            .vertices
            .iter()
            .min_by(|a, b| a.pos.y.total_cmp(&b.pos.y))
            .unwrap();
        let bottom = mesh
            .vertices
            .iter()
            .max_by(|a, b| a.pos.y.total_cmp(&b.pos.y))
            .unwrap();
        assert!(luma(top.color) > luma(bottom.color));
        for v in &mesh.vertices {
            assert!(
                rect.expand(1e-3).contains(v.pos),
                "{:?} outside {rect:?}",
                v.pos
            );
        }
        assert_eq!(mesh.indices.len() % 3, 0);
    }

    #[test]
    fn rounded_outline_handles_tiny_and_huge_radii() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 4.0));
        assert_eq!(rounded_outline(rect, 0.0).len(), 4);
        for p in rounded_outline(rect, 50.0) {
            assert!(rect.expand(1e-3).contains(p));
        }
    }

    #[test]
    fn lighten_and_darken_move_towards_white_and_black() {
        let c = Color32::from_rgb(100, 100, 100);
        assert!(luma(lighten(c, 0.1)) > luma(c));
        assert!(luma(darken(c, 0.1)) < luma(c));
        assert_eq!(lighten(c, 0.0), c);
    }
}
