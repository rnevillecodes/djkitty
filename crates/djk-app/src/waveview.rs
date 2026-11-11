//! Waveform drawing: the scrolling deck lanes and the whole-track overview.
//!
//! Waveforms are drawn the way audio editors do it: one hairline per *physical* pixel,
//! spanning the lowest to the highest sample in that slice of time, so the true shape
//! (transients, asymmetry) shows. Each hairline is tinted by the balance of low, mid and
//! high energy at that moment, so kicks and hats are easy to tell apart.

use djk_analysis::Waveform;
use djk_analysis::waveform::Bin;
use egui::{Color32, Mesh, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::theme;

/// Default seconds of audio visible in a scrolling lane.
pub const DEFAULT_WINDOW_SECS: f64 = 6.0;
/// Zoom limits for the scrolling lanes, in visible seconds.
pub const MIN_WINDOW_SECS: f64 = 1.5;
pub const MAX_WINDOW_SECS: f64 = 24.0;

/// Lowest and highest sample (−1 to 1) over a slice of audio.
pub type Span = (f32, f32);

/// A min/max summary of a track's audio (mono mix) in fixed blocks of frames, built
/// once on load. Fine enough for any lane zoom level (32 frames is under 1 ms).
pub struct Peaks {
    pub rate: u32,
    pub blocks: Vec<Span>,
    /// Display gain: scales the track's loudest sample to full height (at most
    /// [`MAX_DISPLAY_GAIN`]), so quiet tracks stay readable. Display only, not audio.
    pub gain: f32,
}

/// Largest display gain for quiet tracks (8x is +18 dB).
pub const MAX_DISPLAY_GAIN: f32 = 8.0;

fn display_gain(peak: f32) -> f32 {
    if peak <= 1e-6 {
        1.0
    } else {
        (0.98 / peak).clamp(1.0, MAX_DISPLAY_GAIN)
    }
}

impl Peaks {
    /// Frames summarised by each block.
    pub const BLOCK: usize = 32;

    /// Summarise interleaved stereo audio.
    pub fn from_stereo(samples: &[f32], rate: u32) -> Self {
        let blocks = samples
            .chunks(Self::BLOCK * 2)
            .map(|chunk| {
                chunk
                    .chunks_exact(2)
                    .map(|f| 0.5 * (f[0] + f[1]))
                    .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
            })
            .map(|(lo, hi)| if lo > hi { (0.0, 0.0) } else { (lo, hi) })
            .collect::<Vec<Span>>();
        let peak = blocks
            .iter()
            .fold(0.0f32, |m, &(lo, hi)| m.max(lo.abs()).max(hi.abs()));
        Self {
            rate,
            blocks,
            gain: display_gain(peak),
        }
    }

    pub fn duration_secs(&self) -> f64 {
        (self.blocks.len() * Self::BLOCK) as f64 / f64::from(self.rate.max(1))
    }

    /// Min/max over `[t0, t1)` seconds (at least one block), or `None` outside the track.
    fn span(&self, t0: f64, t1: f64) -> Option<Span> {
        let per_sec = f64::from(self.rate) / Self::BLOCK as f64;
        let n = self.blocks.len();
        let (f0, f1) = (t0 * per_sec, t1 * per_sec);
        if f1 < 0.0 || f0 >= n as f64 || n == 0 {
            return None;
        }
        let i0 = f0.floor().max(0.0) as usize;
        let i1 = (f1.ceil() as usize).clamp(i0 + 1, n);
        let (lo, hi) = merge_spans(&self.blocks[i0..i1]);
        Some((lo * self.gain, hi * self.gain))
    }
}

fn merge_spans(spans: &[Span]) -> Span {
    spans
        .iter()
        .fold((0.0f32, 0.0f32), |(lo, hi), &(l, h)| (lo.min(l), hi.max(h)))
}

/// One overview column: its min/max and the band balance used for its tint.
/// `bands` holds each band's level relative to its typical level in the track (see
/// [`band_norms`]), so 1.0 means "as loud as this band usually is".
#[derive(Clone, Copy, Debug, Default)]
pub struct WaveCol {
    pub span: Span,
    pub bands: [f32; 3],
}

/// Each band's typical level in a track: its 90th-percentile peak. Tinting by level
/// relative to this (rather than raw level) stops whichever band a track's mastering
/// makes loudest from colouring everything; instead each moment shows which band is
/// standing out *for this track* (kicks blue, hats mint, vocals and synths amber).
pub fn band_norms(bins: &[Bin]) -> [f32; 3] {
    std::array::from_fn(|k| {
        let mut v: Vec<u8> = bins.iter().map(|b| b[k]).collect();
        if v.is_empty() {
            return 1.0;
        }
        let idx = (v.len() * 9 / 10).min(v.len() - 1);
        let (_, p90, _) = v.select_nth_unstable(idx);
        f32::from(*p90).max(1.0)
    })
}

fn relative(b: [f32; 3], norms: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| b[k] / norms[k])
}

/// About `n` overview columns from a deck's audio summary and band data.
pub fn overview_cols(peaks: &Peaks, bands: &Waveform, n: usize) -> Vec<WaveCol> {
    let total = peaks.blocks.len();
    if total == 0 || n == 0 {
        return Vec::new();
    }
    let norms = band_norms(&bands.bins);
    let per = total.div_ceil(n).max(1);
    let dur = peaks.duration_secs().max(1e-9);
    peaks
        .blocks
        .chunks(per)
        .enumerate()
        .map(|(i, chunk)| {
            let t0 = (i * per * Peaks::BLOCK) as f64 / f64::from(peaks.rate);
            let t1 = t0 + (chunk.len() * Peaks::BLOCK) as f64 / f64::from(peaks.rate);
            let (lo, hi) = merge_spans(chunk);
            let b = mean_bands(bands, t0.min(dur), t1.min(dur)).unwrap_or_default();
            WaveCol {
                span: (lo * peaks.gain, hi * peaks.gain),
                bands: relative(b, norms),
            }
        })
        .collect()
}

/// Overview columns from band data alone (used where the audio isn't loaded, like the
/// track details panel): the loudest band stands in for the min/max.
pub fn cols_from_bins(bins: &[Bin]) -> Vec<WaveCol> {
    let loudest = |b: &Bin| f32::from(b[0].max(b[1]).max(b[2])) / 255.0;
    let gain = display_gain(bins.iter().map(loudest).fold(0.0, f32::max));
    let norms = band_norms(bins);
    bins.iter()
        .map(|b| {
            let m = loudest(b) * gain;
            WaveCol {
                span: (-m, m),
                bands: relative(b.map(f32::from), norms),
            }
        })
        .collect()
}

/// How strongly the most prominent band dominates the tint. A plain average (1) blends
/// most of a track into the same muddy colour; a higher power lets kicks read clearly as
/// low, hats as high and so on, while still shading genuinely mixed moments in between.
const TINT_SHARPNESS: i32 = 4;

/// Tint for a slice of audio, from each band's level relative to its typical level
/// (`rel`, 1.0 = typical): the band colours blended by those levels, sharpened towards
/// the band that stands out most.
pub fn band_colour(rel: [f32; 3]) -> Color32 {
    let w = rel.map(|v| v.max(0.0).powi(TINT_SHARPNESS));
    let sum: f32 = w.iter().sum();
    if sum <= 1e-6 {
        return theme::BAND_HIGH;
    }
    let colours = [theme::BAND_LOW, theme::BAND_MID, theme::BAND_HIGH];
    let ch = |f: fn(&Color32) -> u8| {
        let v: f32 = colours
            .iter()
            .zip(w)
            .map(|(c, wk)| f32::from(f(c)) * wk)
            .sum();
        (v / sum).round() as u8
    };
    Color32::from_rgb(ch(Color32::r), ch(Color32::g), ch(Color32::b))
}

/// Mean band levels over `[t0, t1)` seconds (at least one bin), for overview tints.
fn mean_bands(wf: &Waveform, t0: f64, t1: f64) -> Option<[f32; 3]> {
    let bps = f64::from(wf.bins_per_sec);
    let n = wf.bins.len();
    let (f0, f1) = (t0 * bps, t1 * bps);
    if f1 < 0.0 || f0 >= n as f64 || n == 0 {
        return None;
    }
    let i0 = f0.floor().max(0.0) as usize;
    let i1 = (f1.ceil() as usize).clamp(i0 + 1, n);
    let bins = &wf.bins[i0..i1];
    let sum = bins.iter().fold([0.0f32; 3], |a, b| {
        [
            a[0] + f32::from(b[0]),
            a[1] + f32::from(b[1]),
            a[2] + f32::from(b[2]),
        ]
    });
    Some(sum.map(|v| v / bins.len() as f32))
}

/// Band peaks over `[t0, t1)` seconds: the peak of the covered bins when the span covers
/// at least one bin, otherwise a linear interpolation between neighbours.
fn sample(wf: &Waveform, t0: f64, t1: f64) -> Option<[f32; 3]> {
    let bps = f64::from(wf.bins_per_sec);
    let n = wf.bins.len();
    let f0 = t0 * bps;
    let f1 = t1 * bps;
    if f1 < 0.0 || f0 >= n as f64 || n == 0 {
        return None;
    }
    if f1 - f0 >= 1.0 {
        let i0 = f0.floor().max(0.0) as usize;
        let i1 = (f1.ceil() as usize).clamp(i0 + 1, n);
        let m = wf.bins[i0..i1].iter().fold([0u8; 3], |a, b| {
            [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
        });
        return Some(m.map(f32::from));
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

/// Add one hairline column spanning `span` (−1 to 1, positive up) around `mid_y`.
/// Silence still draws a one-pixel line so the waveform reads as continuous.
#[allow(clippy::too_many_arguments)]
fn hairline(
    mesh: &mut Mesh,
    x0: f32,
    w: f32,
    mid_y: f32,
    half_h: f32,
    span: Span,
    colour: Color32,
) {
    let mut top = mid_y - span.1.clamp(-1.0, 1.0) * half_h;
    let mut bottom = mid_y - span.0.clamp(-1.0, 1.0) * half_h;
    if bottom - top < w {
        let c = 0.5 * (top + bottom);
        top = c - 0.5 * w;
        bottom = c + 0.5 * w;
    }
    mesh.add_colored_rect(
        Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x0 + w, bottom)),
        colour,
    );
}

/// Scrolling waveform lane centred on a fixed playhead at `position_secs`.
///
/// Returns the lane's response; hovering it and scrolling changes `window_secs` (zoom).
#[allow(clippy::too_many_arguments)]
pub fn scrolling(
    ui: &mut Ui,
    size: Vec2,
    peaks: Option<&Peaks>,
    bands: Option<&Waveform>,
    band_norm: [f32; 3],
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

    if let Some(p) = peaks {
        let mut mesh = Mesh::default();
        for c in 0..cols {
            let t0 = start + c as f64 * secs_per_col;
            let t1 = t0 + secs_per_col;
            let Some(span) = p.span(t0, t1) else {
                if t0 >= p.duration_secs() {
                    break;
                }
                continue;
            };
            let b = bands.and_then(|w| sample(w, t0, t1)).unwrap_or_default();
            // Already-played audio is dimmed.
            let dim = if t1 < position_secs { 0.5 } else { 1.0 };
            let x0 = rect.left() + c as f32 * col_w;
            hairline(
                &mut mesh,
                x0,
                col_w,
                mid_y,
                half_h,
                span,
                band_colour(relative(b, band_norm)).gamma_multiply(dim),
            );
        }
        painter.add(egui::Shape::mesh(mesh));
    } else {
        // Next to the deck letter in the top-left corner.
        painter.text(
            rect.left_top() + Vec2::new(28.0, 6.0),
            egui::Align2::LEFT_TOP,
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
    cols_in: &[WaveCol],
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
    let half_h = rect.height() * 0.46;
    if !cols_in.is_empty() {
        let mut mesh = Mesh::default();
        let n = cols_in.len();
        let ppp = ui.ctx().pixels_per_point();
        let cols = (rect.width().max(1.0) * ppp).ceil() as usize;
        let col_w = 1.0 / ppp;
        for c in 0..cols {
            let i0 = c * n / cols;
            let i1 = ((c + 1) * n / cols).clamp(i0 + 1, n);
            if i0 >= n {
                break;
            }
            let slice = &cols_in[i0..i1];
            let span = slice.iter().fold((0.0f32, 0.0f32), |(lo, hi), w| {
                (lo.min(w.span.0), hi.max(w.span.1))
            });
            // Average (not peak) band levels: spiky hats would otherwise win every column.
            let sum = slice.iter().fold([0.0f32; 3], |a, w| {
                [a[0] + w.bands[0], a[1] + w.bands[1], a[2] + w.bands[2]]
            });
            let bands = sum.map(|v| v / slice.len() as f32);
            let frac = (c as f64 + 0.5) / cols as f64;
            let dim = if position_frac.is_some_and(|p| frac < p) {
                0.5
            } else {
                1.0
            };
            let x0 = rect.left() + c as f32 * col_w;
            hairline(
                &mut mesh,
                x0,
                col_w,
                mid_y,
                half_h,
                span,
                band_colour(bands).gamma_multiply(dim),
            );
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

    #[test]
    fn peaks_keep_signed_min_and_max_per_block() {
        // One block of a positive-heavy wave, one of silence.
        let mut s = Vec::new();
        for i in 0..Peaks::BLOCK {
            let v = if i == 3 {
                0.9
            } else if i == 7 {
                -0.4
            } else {
                0.1
            };
            s.extend([v, v]);
        }
        s.extend(std::iter::repeat_n(0.0, Peaks::BLOCK * 2));
        let p = Peaks::from_stereo(&s, 44_100);
        assert_eq!(p.blocks.len(), 2);
        assert_eq!(p.blocks[0], (-0.4, 0.9));
        assert_eq!(p.blocks[1], (0.0, 0.0));
        let secs = Peaks::BLOCK as f64 / 44_100.0;
        let (lo, hi) = p.span(0.0, 2.0 * secs).unwrap();
        assert!((lo - -0.4 * p.gain).abs() < 1e-6 && (hi - 0.9 * p.gain).abs() < 1e-6);
        assert!(p.span(-1.0, -0.5).is_none());
        assert!(p.span(10.0, 11.0).is_none());
    }

    #[test]
    fn display_gain_lifts_quiet_tracks_within_limits() {
        assert_eq!(display_gain(1.0), 1.0);
        assert!((display_gain(0.49) - 2.0).abs() < 1e-3);
        assert_eq!(display_gain(0.01), MAX_DISPLAY_GAIN);
        assert_eq!(display_gain(0.0), 1.0);
    }

    #[test]
    fn band_colour_follows_the_dominant_band() {
        assert_eq!(band_colour([1.5, 0.0, 0.0]), theme::BAND_LOW);
        assert_eq!(band_colour([0.0, 0.0, 1.5]), theme::BAND_HIGH);
        // An even mix lands between the band colours.
        let c = band_colour([1.0, 1.0, 1.0]);
        assert_ne!(c, theme::BAND_LOW);
        assert_ne!(c, theme::BAND_HIGH);
        // A band half again above the others dominates clearly.
        let c = band_colour([1.5, 1.0, 1.0]);
        let dist = |a: Color32, b: Color32| {
            (i32::from(a.r()) - i32::from(b.r())).abs()
                + (i32::from(a.g()) - i32::from(b.g())).abs()
                + (i32::from(a.b()) - i32::from(b.b())).abs()
        };
        assert!(dist(c, theme::BAND_LOW) < dist(c, theme::BAND_MID));
        assert!(dist(c, theme::BAND_LOW) < dist(c, theme::BAND_HIGH));
    }

    #[test]
    fn norms_make_a_band_that_is_always_loud_look_typical() {
        // Lows always at 200, mids at 40, highs at 40: raw levels would colour it all low.
        let mut bins = vec![[200u8, 40, 40]; 100];
        // One hat hit: highs jump to 120 (3x their norm) while lows stay at their norm.
        bins[50] = [200, 40, 120];
        let n = band_norms(&bins);
        assert_eq!(n, [200.0, 40.0, 40.0]);
        let typical = relative([200.0, 40.0, 40.0], n);
        assert_eq!(typical, [1.0, 1.0, 1.0]);
        let hat = band_colour(relative([200.0, 40.0, 120.0], n));
        let dist = |a: Color32, b: Color32| {
            (i32::from(a.r()) - i32::from(b.r())).abs()
                + (i32::from(a.g()) - i32::from(b.g())).abs()
                + (i32::from(a.b()) - i32::from(b.b())).abs()
        };
        assert!(dist(hat, theme::BAND_HIGH) < 30, "{hat:?}");
    }
}
