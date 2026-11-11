//! Waveform drawing: the scrolling deck lanes and the whole-track overview.
//!
//! Both are drawn one column per *physical* pixel, so they stay sharp on high-DPI screens.

use djk_analysis::Waveform;
use djk_analysis::waveform::Bin;
use egui::{Color32, Mesh, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::theme;

/// Default seconds of audio visible in a scrolling lane.
pub const DEFAULT_WINDOW_SECS: f64 = 6.0;
/// Zoom limits for the scrolling lanes, in visible seconds.
pub const MIN_WINDOW_SECS: f64 = 1.5;
pub const MAX_WINDOW_SECS: f64 = 24.0;

/// Visual amplitude curve for a stored 0–255 peak (lifts quiet detail).
fn amp(v: f32) -> f32 {
    (v / 255.0).sqrt()
}

/// Add one column of three stacked band bars centred on `mid_y`.
fn column(mesh: &mut Mesh, x0: f32, x1: f32, mid_y: f32, half_h: f32, b: [f32; 3], dim: f32) {
    let layers = [
        (amp(b[0]), theme::BAND_LOW),
        (amp(b[1]) * 0.8, theme::BAND_MID),
        (amp(b[2]) * 0.55, theme::BAND_HIGH),
    ];
    for (a, c) in layers {
        if a <= 0.01 {
            continue;
        }
        let h = a * half_h;
        let rect = Rect::from_min_max(Pos2::new(x0, mid_y - h), Pos2::new(x1, mid_y + h));
        mesh.add_colored_rect(rect, c.gamma_multiply(dim));
    }
}

fn max_bins(bins: &[Bin]) -> [f32; 3] {
    let m = bins.iter().fold([0u8; 3], |a, b| {
        [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
    });
    m.map(f32::from)
}

/// Value of the waveform over `[t0, t1)` seconds: the peak of the covered bins when the
/// span covers at least one bin, otherwise a linear interpolation between neighbours, so
/// zoomed-in views are smooth rather than stepped.
fn sample(wf: &Waveform, t0: f64, t1: f64) -> Option<[f32; 3]> {
    let bps = f64::from(wf.bins_per_sec);
    let n = wf.bins.len();
    let f0 = t0 * bps;
    let f1 = t1 * bps;
    if f1 < 0.0 || f0 >= n as f64 {
        return None;
    }
    if f1 - f0 >= 1.0 {
        let i0 = f0.floor().max(0.0) as usize;
        let i1 = (f1.ceil() as usize).clamp(i0 + 1, n);
        return Some(max_bins(&wf.bins[i0..i1]));
    }
    let centre = (0.5 * (f0 + f1) - 0.5).clamp(0.0, (n - 1) as f64);
    let i = centre.floor() as usize;
    let j = (i + 1).min(n - 1);
    let frac = (centre - i as f64) as f32;
    let (a, b) = (wf.bins[i], wf.bins[j]);
    Some(std::array::from_fn(|k| {
        f32::from(a[k]) + (f32::from(b[k]) - f32::from(a[k])) * frac
    }))
}

/// Scrolling waveform lane centred on a fixed playhead at `position_secs`.
///
/// Returns the lane's response; hovering it and scrolling changes `window_secs` (zoom).
#[allow(clippy::too_many_arguments)]
pub fn scrolling(
    ui: &mut Ui,
    size: Vec2,
    waveform: Option<&Waveform>,
    position_secs: f64,
    cue_secs: Option<f64>,
    accent: Color32,
    label: &str,
    window_secs: &mut f64,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = (-f64::from(scroll) * 0.004).exp();
            *window_secs = (*window_secs * factor).clamp(MIN_WINDOW_SECS, MAX_WINDOW_SECS);
        }
    }
    let window = *window_secs;

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme::BG);
    let mid_y = rect.center().y;
    let half_h = rect.height() * 0.46;
    let width = rect.width().max(1.0);
    let ppp = ui.ctx().pixels_per_point();
    let cols = (width * ppp).ceil() as usize;
    let col_w = 1.0 / ppp;
    let secs_per_col = window / cols.max(1) as f64;
    let start = position_secs - window / 2.0;

    // Faint centre line so silence still reads as a lane.
    painter.line_segment(
        [
            Pos2::new(rect.left(), mid_y),
            Pos2::new(rect.right(), mid_y),
        ],
        Stroke::new(1.0, theme::RAISED),
    );

    if let Some(wf) = waveform {
        let mut mesh = Mesh::default();
        for c in 0..cols {
            let t0 = start + c as f64 * secs_per_col;
            let t1 = t0 + secs_per_col;
            let Some(b) = sample(wf, t0, t1) else {
                if t0 >= wf.duration_secs() {
                    break;
                }
                continue;
            };
            // Already-played audio is dimmed slightly.
            let dim = if t1 < position_secs { 0.55 } else { 1.0 };
            let x0 = rect.left() + c as f32 * col_w;
            column(&mut mesh, x0, x0 + col_w, mid_y, half_h, b, dim);
        }
        painter.add(egui::Shape::mesh(mesh));
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No track loaded",
            egui::FontId::proportional(13.0),
            theme::TEXT_DIM,
        );
    }

    if let Some(cue) = cue_secs {
        let x = rect.left() + ((cue - start) / window) as f32 * width;
        if (rect.left()..=rect.right()).contains(&x) {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.5, theme::CUE_MARKER),
            );
        }
    }
    // Fixed playhead.
    let cx = rect.center().x;
    painter.line_segment(
        [Pos2::new(cx, rect.top()), Pos2::new(cx, rect.bottom())],
        Stroke::new(2.0, theme::PLAYHEAD),
    );
    // Deck letter, so stacked lanes are easy to tell apart.
    painter.text(
        rect.left_top() + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(16.0),
        accent,
    );
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, accent.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    response
}

/// Result of interacting with an overview.
pub struct OverviewResponse {
    /// Fraction of the track clicked (0–1), if clicked.
    pub clicked_at: Option<f64>,
    pub response: egui::Response,
}

/// Whole-track overview with position and cue markers.
pub fn overview(
    ui: &mut Ui,
    size: Vec2,
    bins: &[Bin],
    position_frac: Option<f64>,
    cue_frac: Option<f64>,
    accent: Color32,
    clickable: bool,
) -> OverviewResponse {
    let sense = if clickable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, theme::BG);
    let mid_y = rect.center().y;
    let half_h = rect.height() * 0.45;
    if !bins.is_empty() {
        let mut mesh = Mesh::default();
        let n = bins.len();
        let ppp = ui.ctx().pixels_per_point();
        let cols = (rect.width().max(1.0) * ppp).ceil() as usize;
        let col_w = 1.0 / ppp;
        for c in 0..cols {
            let i0 = c * n / cols;
            let i1 = ((c + 1) * n / cols).clamp(i0 + 1, n);
            if i0 >= n {
                break;
            }
            let b = max_bins(&bins[i0..i1]);
            let frac = (c as f64 + 0.5) / cols as f64;
            let dim = if position_frac.is_some_and(|p| frac < p) {
                0.5
            } else {
                1.0
            };
            let x0 = rect.left() + c as f32 * col_w;
            column(&mut mesh, x0, x0 + col_w, mid_y, half_h, b, dim);
        }
        painter.add(egui::Shape::mesh(mesh));
    }
    let marker = |frac: f64, colour: Color32, width: f32| {
        let x = rect.left() + rect.width() * frac.clamp(0.0, 1.0) as f32;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(width, colour),
        );
    };
    if let Some(c) = cue_frac {
        marker(c, theme::CUE_MARKER, 1.5);
    }
    if let Some(p) = position_frac {
        marker(p, accent, 2.0);
    }
    let clicked_at = if clickable && response.clicked() {
        response
            .interact_pointer_pos()
            .map(|p| f64::from((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
    } else {
        None
    };
    OverviewResponse {
        clicked_at,
        response,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wf(bins: Vec<Bin>) -> Waveform {
        Waveform {
            bins_per_sec: 10,
            bins,
        }
    }

    #[test]
    fn wide_spans_take_the_peak() {
        let w = wf(vec![[10, 0, 0], [200, 0, 0], [30, 0, 0], [0, 0, 0]]);
        assert_eq!(sample(&w, 0.0, 0.3).unwrap()[0], 200.0);
    }

    #[test]
    fn narrow_spans_interpolate() {
        let w = wf(vec![[0, 0, 0], [100, 0, 0]]);
        // Half way between the centres of bin 0 (0.05 s) and bin 1 (0.15 s).
        let v = sample(&w, 0.099, 0.101).unwrap()[0];
        assert!((v - 50.0).abs() < 2.0, "{v}");
    }

    #[test]
    fn outside_the_track_is_empty() {
        let w = wf(vec![[1, 1, 1]; 10]);
        assert!(sample(&w, -2.0, -1.0).is_none());
        assert!(sample(&w, 5.0, 5.1).is_none());
    }
}
