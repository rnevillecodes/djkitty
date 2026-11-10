//! Waveform drawing: the scrolling deck view and the whole-track overview.

use djk_analysis::Waveform;
use djk_analysis::waveform::Bin;
use egui::{Color32, Mesh, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::theme;

/// Seconds of audio visible in the scrolling view.
pub const WINDOW_SECS: f64 = 8.0;

/// Visual amplitude curve for a stored 0–255 peak (lifts quiet detail).
fn amp(v: u8) -> f32 {
    (f32::from(v) / 255.0).sqrt()
}

/// Add one column of three stacked band bars centred on `mid_y`.
fn column(mesh: &mut Mesh, x0: f32, x1: f32, mid_y: f32, half_h: f32, b: Bin, dim: f32) {
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

/// Scrolling waveform centred on a fixed playhead at `position_secs`.
pub fn scrolling(
    ui: &mut Ui,
    size: Vec2,
    waveform: Option<&Waveform>,
    position_secs: f64,
    cue_secs: Option<f64>,
    accent: Color32,
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme::BG);
    let mid_y = rect.center().y;
    let half_h = rect.height() * 0.46;
    let width = rect.width().max(1.0);
    let secs_per_px = WINDOW_SECS / f64::from(width);
    let start = position_secs - WINDOW_SECS / 2.0;

    if let Some(wf) = waveform {
        let bps = f64::from(wf.bins_per_sec);
        let mut mesh = Mesh::default();
        let cols = width.ceil() as usize;
        for x in 0..cols {
            let t0 = start + x as f64 * secs_per_px;
            let t1 = t0 + secs_per_px;
            if t1 < 0.0 {
                continue;
            }
            let i0 = (t0 * bps).floor().max(0.0) as usize;
            let i1 = ((t1 * bps).ceil() as usize).max(i0 + 1);
            if i0 >= wf.bins.len() {
                break;
            }
            let b = wf.bins[i0..i1.min(wf.bins.len())]
                .iter()
                .fold([0u8; 3], |a, b| {
                    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
                });
            // Already-played audio is dimmed slightly.
            let dim = if t1 < position_secs { 0.55 } else { 1.0 };
            let x0 = rect.left() + x as f32;
            column(&mut mesh, x0, x0 + 1.0, mid_y, half_h, b, dim);
        }
        painter.add(egui::Shape::mesh(mesh));
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No waveform",
            egui::FontId::proportional(13.0),
            theme::TEXT_DIM,
        );
    }

    if let Some(cue) = cue_secs {
        let x = rect.left() + ((cue - start) / secs_per_px) as f32;
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
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, accent.gamma_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
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
        let w = rect.width() / n as f32;
        for (i, &b) in bins.iter().enumerate() {
            let frac = (i as f64 + 0.5) / n as f64;
            let dim = if position_frac.is_some_and(|p| frac < p) {
                0.5
            } else {
                1.0
            };
            let x0 = rect.left() + i as f32 * w;
            column(&mut mesh, x0, x0 + w.max(1.0), mid_y, half_h, b, dim);
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
