//! Window layout and widgets: stacked full-width waveform lanes on top, then two decks
//! around a mixer strip, then the browser.

use std::time::{Duration, Instant};

use djk_controller::FaderTarget;
use djk_core::{Bpm, Command, DeckId, DeckState, EngineState, MixerState, PITCH_RANGE};
use djk_library::SortKey;
use egui::{Align, Color32, Layout, Rect, RichText, Sense, Ui, Vec2};
use egui_extras::{Column, TableBuilder};

use crate::app::{DjApp, View};
use crate::theme;
use crate::util::fmt_time;
use crate::waveview;
use crate::widgets;

/// Height of the row holding the two deck panels and the mixer.
const DECKS_ROW_HEIGHT: f32 = 222.0;
const MIXER_WIDTH: f32 = 250.0;
const MESSAGE_SECS: u64 = 5;
/// Gap between the stacked waveform lanes.
const LANE_GAP: f32 = 4.0;
/// Space kept for the library below the decks when sizing the lanes.
const MIN_BROWSER_HEIGHT: f32 = 280.0;

pub fn layout(app: &mut DjApp, ui: &mut Ui) {
    egui::Panel::bottom("status")
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(10, 4)),
        )
        .show(ui, |ui| status_bar(app, ui));
    // Lanes grow with the window, but never squeeze the library below its minimum.
    // (Three quarters of the space they could take, leaving more room for the library.)
    let lane_h = ((ui.available_height() - DECKS_ROW_HEIGHT - MIN_BROWSER_HEIGHT - 40.0) / 2.0)
        .clamp(80.0, 170.0)
        * 0.75;
    egui::Panel::top("decks")
        .resizable(false)
        .exact_size(2.0 * lane_h + LANE_GAP + 8.0 + DECKS_ROW_HEIGHT)
        .frame(egui::Frame::new().fill(theme::BG).inner_margin(8))
        .show(ui, |ui| {
            waveform_lanes(app, ui, lane_h);
            ui.add_space(8.0);
            decks_row(app, ui);
        });
    egui::Panel::left("sidebar")
        .resizable(true)
        .default_size(230.0)
        .size_range(190.0..=360.0)
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(10, 4)),
        )
        .show(ui, |ui| sidebar(app, ui));
    // Track details only take space while a track is selected.
    if app.selected_row().is_some() {
        egui::Panel::right("details")
            .resizable(true)
            .default_size(270.0)
            .size_range(230.0..=400.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("details")
                    .auto_shrink([false, false])
                    .show(ui, |ui| track_details(app, ui));
            });
    }
    egui::CentralPanel::default().show(ui, |ui| browser(app, ui));
    if app.show_shortcuts {
        shortcuts_window(app, ui.ctx());
    }
    if app.new_set_dialog {
        new_set_dialog(app, ui.ctx());
    }
}

// ---------------------------------------------------------------- decks + mixer

/// Both decks' scrolling waveforms, full width and stacked (A above B) on a shared time
/// scale, so beats can be lined up by eye.
fn waveform_lanes(app: &mut DjApp, ui: &mut Ui, lane_h: f32) {
    let state = app.audio.state;
    let now = Instant::now();
    let width = ui.available_width().max(1.0);
    ui.spacing_mut().item_spacing.y = LANE_GAP;
    for deck in DeckId::ALL {
        let s = *state.deck(deck);
        let pos = display_position_secs(&state, &s, now);
        let d = &app.decks[deck.index()];
        let peaks = d.peaks.clone().filter(|_| s.loaded);
        let bands = d.waveform.clone().filter(|_| s.loaded);
        let cue_secs = s.loaded.then(|| s.cue / f64::from(s.track_rate.max(1)));
        let lane = waveview::scrolling(
            ui,
            Vec2::new(width, lane_h),
            peaks.as_deref(),
            bands.as_deref(),
            pos,
            cue_secs,
            theme::deck_colour(deck),
            deck.label(),
            &mut app.wave_window_secs,
        );
        let rect = lane.rect;
        lane.on_hover_text(format!(
            "Showing {:.1} s · scroll to zoom",
            app.wave_window_secs
        ));
        track_drop_target(app, ui, rect, |_| deck);
    }
}

fn decks_row(app: &mut DjApp, ui: &mut Ui) {
    let full = ui.available_size();
    use egui_extras::{Size, StripBuilder};
    if full.x < MIXER_WIDTH + 400.0 {
        return; // window not laid out yet
    }
    StripBuilder::new(ui)
        .size(Size::remainder().at_least(200.0))
        .size(Size::exact(MIXER_WIDTH))
        .size(Size::remainder().at_least(200.0))
        .horizontal(|mut strip| {
            strip.cell(|ui| deck_panel(app, ui, DeckId::A));
            strip.cell(|ui| mixer_strip(app, ui));
            strip.cell(|ui| deck_panel(app, ui, DeckId::B));
        });
}

/// Playhead position as heard: the snapshot position extrapolated to now and moved back
/// by the output latency, so the waveform matches the audio.
pub fn display_position_secs(state: &EngineState, d: &DeckState, now: Instant) -> f64 {
    let base = d.position_secs();
    if !d.playing {
        return base;
    }
    let Some(ts) = state.timestamp else {
        return base;
    };
    let elapsed = now.saturating_duration_since(ts).as_secs_f64();
    (base + (elapsed - state.output_latency) * d.speed).clamp(0.0, d.duration_secs())
}

/// A panel with a soft top-lit surface and a thin accent strip along the top edge.
fn card(ui: &mut Ui, accent: Color32, add: impl FnOnce(&mut Ui)) -> Rect {
    let background = ui.painter().add(egui::Shape::Noop);
    let rect = egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x2a, 0x2e, 0x37)))
        .corner_radius(10)
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 10,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size().max(Vec2::ZERO));
            add(ui)
        })
        .response
        .rect;

    // Paint the background behind the content, now that the size is known.
    let mut shapes = vec![egui::Shape::rect_filled(rect, 10, theme::PANEL)];
    let sheen = Rect::from_min_size(
        rect.min + Vec2::new(6.0, 1.0),
        Vec2::new(rect.width() - 12.0, 46.0),
    );
    let strip = Rect::from_min_size(
        rect.min + Vec2::new(14.0, 0.0),
        Vec2::new(rect.width() - 28.0, 2.0),
    );
    let mut mesh = egui::Mesh::default();
    for (r, top, bottom) in [
        (sheen, Color32::from_white_alpha(7), Color32::TRANSPARENT),
        (
            Rect::from_min_size(strip.min, Vec2::new(strip.width(), 18.0)),
            accent.gamma_multiply(0.12),
            Color32::TRANSPARENT,
        ),
    ] {
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(r.left_top(), top);
        mesh.colored_vertex(r.right_top(), top);
        mesh.colored_vertex(r.right_bottom(), bottom);
        mesh.colored_vertex(r.left_bottom(), bottom);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    shapes.push(egui::Shape::mesh(mesh));
    shapes.push(egui::Shape::rect_filled(
        strip,
        1,
        accent.gamma_multiply(0.85),
    ));
    ui.painter().set(background, egui::Shape::Vec(shapes));
    rect
}

/// Drag-and-drop payload for a library track being dragged onto a deck.
#[derive(Clone)]
pub struct TrackDrag {
    pub id: djk_core::TrackId,
    pub title: String,
    /// The row it came from, when dragged within a set (so it can be reordered).
    pub from_row: Option<usize>,
    /// False for missing files: they can be reordered but not loaded.
    pub loadable: bool,
}

/// Make `rect` a drop target for library tracks. `deck_at` picks the deck for a pointer
/// position (the mixer uses its left and right halves). While a track is dragged over it
/// the target is outlined in the deck's colour; releasing loads the track.
fn track_drop_target(
    app: &mut DjApp,
    ui: &Ui,
    rect: egui::Rect,
    deck_at: impl Fn(egui::Pos2) -> DeckId,
) {
    let ctx = ui.ctx();
    let Some(drag) = egui::DragAndDrop::payload::<TrackDrag>(ctx).filter(|d| d.loadable) else {
        return;
    };
    let Some(pos) = ctx.pointer_latest_pos().filter(|p| rect.contains(*p)) else {
        return;
    };
    let deck = deck_at(pos);
    let accent = theme::deck_colour(deck);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("track_drop_highlight"),
    ));
    painter.rect_filled(rect, 8.0, accent.gamma_multiply(0.08));
    painter.rect_stroke(
        rect,
        8.0,
        egui::Stroke::new(2.0, accent),
        egui::StrokeKind::Inside,
    );
    if ctx.input(|i| i.pointer.any_released()) {
        egui::DragAndDrop::clear_payload(ctx);
        app.load_track(deck, drag.id);
    }
}

fn deck_panel(app: &mut DjApp, ui: &mut Ui, deck: DeckId) {
    let accent = theme::deck_colour(deck);
    let state = app.audio.state;
    let s = *state.deck(deck);
    let now = Instant::now();
    let blink = (app.start_time.elapsed().as_millis() / 500) % 2 == 0;
    let rect = card(ui, accent, |ui| {
        let full = ui.available_rect_before_wrap();
        let pitch_w = 58.0;
        let main = Rect::from_min_max(
            full.min,
            egui::pos2(full.right() - pitch_w - 10.0, full.bottom()),
        );
        let pitch_col =
            Rect::from_min_max(egui::pos2(full.right() - pitch_w, full.top()), full.max);

        ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| {
            deck_main(app, ui, deck, &state, &s, now, blink);
        });
        ui.scope_builder(egui::UiBuilder::new().max_rect(pitch_col), |ui| {
            pitch_column(app, ui, deck, &s);
        });
    });
    track_drop_target(app, ui, rect, |_| deck);
}

/// The deck's left part: header with the BPM screen, status, overview and transport.
fn deck_main(
    app: &mut DjApp,
    ui: &mut Ui,
    deck: DeckId,
    state: &EngineState,
    s: &DeckState,
    now: Instant,
    blink: bool,
) {
    let accent = theme::deck_colour(deck);
    let width = ui.available_width();
    let info = app.decks[deck.index()]
        .track
        .as_ref()
        .filter(|_| s.loaded || app.decks[deck.index()].loading)
        .cloned();

    // Header: deck badge, title/artist, and the BPM/key screen.
    let screen_w = 132.0;
    ui.horizontal(|ui| {
        let (badge, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
        let p = ui.painter();
        p.circle_filled(badge.center(), 16.0, accent.gamma_multiply(0.18));
        p.circle_stroke(badge.center(), 16.0, egui::Stroke::new(1.5, accent));
        p.text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            deck.label(),
            egui::FontId::proportional(18.0),
            accent,
        );
        ui.vertical(|ui| {
            ui.set_width((width - screen_w - 56.0).max(40.0));
            ui.spacing_mut().item_spacing.y = 1.0;
            match &info {
                Some(t) => {
                    ui.add(
                        egui::Label::new(RichText::new(&t.title).size(16.0).strong()).truncate(),
                    );
                    let artist = if t.artist.is_empty() {
                        "—"
                    } else {
                        &t.artist
                    };
                    ui.add(
                        egui::Label::new(RichText::new(artist).color(theme::TEXT_DIM)).truncate(),
                    );
                }
                None => {
                    ui.label(
                        RichText::new("No track loaded")
                            .size(16.0)
                            .color(theme::TEXT_DIM),
                    );
                    ui.label(
                        RichText::new(match deck {
                            DeckId::A => "Drag a track here, or select one and press Shift+Left",
                            DeckId::B => "Drag a track here, or select one and press Shift+Right",
                        })
                        .small()
                        .color(theme::TEXT_DIM),
                    );
                }
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(screen_w, 44.0), Sense::hover());
            let p = ui.painter();
            let inner = widgets::screen(p, r);
            let key = info.as_ref().and_then(|t| t.key);
            let bpm = info.as_ref().and_then(|t| t.bpm);
            let bpm_text = bpm
                .map(|b| format!("{}", Bpm(b).with_pitch(s.pitch)))
                .unwrap_or_else(|| "—".into());
            p.text(
                egui::pos2(inner.right(), inner.top() - 3.0),
                egui::Align2::RIGHT_TOP,
                bpm_text,
                egui::FontId::monospace(20.0),
                theme::TEXT,
            );
            p.text(
                egui::pos2(inner.left(), inner.top() - 1.0),
                egui::Align2::LEFT_TOP,
                "BPM",
                egui::FontId::proportional(9.0),
                theme::TEXT_DIM,
            );
            let key_text = key.map(|k| k.camelot()).unwrap_or_else(|| "—".into());
            p.text(
                egui::pos2(inner.left(), inner.bottom() + 2.0),
                egui::Align2::LEFT_BOTTOM,
                format!("KEY {key_text}"),
                egui::FontId::proportional(11.0),
                if key.is_some() {
                    theme::KEY
                } else {
                    theme::TEXT_DIM
                },
            );
            let pitch_col = if s.pitch.abs() < 1e-4 {
                theme::TEXT_DIM
            } else {
                accent
            };
            p.text(
                egui::pos2(inner.right(), inner.bottom() + 2.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("{:+.1}%", s.pitch * 100.0),
                egui::FontId::monospace(11.0),
                pitch_col,
            );
            resp.on_hover_text(
                key.map(|k| k.standard())
                    .unwrap_or_else(|| "Key unknown".into()),
            );
        });
    });

    // Status line (fixed height so the layout doesn't jump).
    let d = &mut app.decks[deck.index()];
    if d.message
        .as_ref()
        .is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(MESSAGE_SECS))
    {
        d.message = None;
    }
    ui.allocate_ui(Vec2::new(width, 16.0), |ui| {
        ui.set_height(16.0);
        if d.loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Loading…").small());
            });
        } else if let Some((msg, _)) = &d.message {
            ui.label(RichText::new(msg).small().color(theme::WARN));
        }
    });

    // Overview with click-to-seek while paused.
    let pos = display_position_secs(state, s, now);
    let dur = s.duration_secs().max(1e-6);
    let ov = waveview::overview(
        ui,
        Vec2::new(width, 40.0),
        if s.loaded {
            &app.decks[deck.index()].overview
        } else {
            &[]
        },
        s.loaded.then_some(pos / dur),
        s.loaded.then(|| s.cue / s.track_frames.max(1) as f64),
        accent,
        s.loaded,
    );
    if let Some(frac) = ov.clicked_at {
        if s.playing {
            app.deck_message(deck, "Pause the deck to seek from the overview");
        } else {
            app.audio.send(Command::Seek {
                deck,
                frame: frac * s.track_frames as f64,
            });
        }
    }
    if s.loaded && !s.playing {
        ov.response.on_hover_text("Click to seek");
    }

    // Transport buttons and the time screen.
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let paused_away = s.loaded && !s.playing && !s.at_cue();
        let cue_lit = if s.previewing || (s.loaded && !s.playing && s.at_cue()) {
            1.0
        } else if paused_away && blink {
            0.8
        } else {
            0.0
        };
        let cue_btn = widgets::transport_button(
            ui,
            50.0,
            widgets::TransportIcon::Cue,
            theme::CUE_MARKER,
            cue_lit,
            s.loaded,
        );
        let down = cue_btn.is_pointer_button_down_on();
        let held = &mut app.decks[deck.index()].cue_held;
        if down && !*held {
            *held = true;
            app.audio.send(Command::CuePress(deck));
        } else if !down && *held {
            *held = false;
            app.audio.send(Command::CueRelease(deck));
        }
        cue_btn.on_hover_text("Paused: set cue / hold to preview · Playing: back to cue");

        let play_lit = if s.playing && !s.previewing {
            1.0
        } else if s.loaded && blink {
            0.8
        } else {
            0.0
        };
        let play = widgets::transport_button(
            ui,
            50.0,
            widgets::TransportIcon::PlayPause,
            theme::OK,
            play_lit,
            s.loaded,
        );
        if play.on_hover_text("Play / pause").clicked() {
            app.audio.send(Command::TogglePlay(deck));
        }

        // Time screen.
        let w = ui.available_width().max(60.0);
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, 50.0), Sense::hover());
        let p = ui.painter();
        let inner = widgets::screen(p, r.shrink2(Vec2::new(0.0, 3.0)));
        p.text(
            inner.left_top() + Vec2::new(0.0, -2.0),
            egui::Align2::LEFT_TOP,
            "ELAPSED",
            egui::FontId::proportional(9.0),
            theme::TEXT_DIM,
        );
        p.text(
            egui::pos2(inner.left(), inner.bottom() + 3.0),
            egui::Align2::LEFT_BOTTOM,
            fmt_tenths(pos),
            egui::FontId::monospace(20.0),
            theme::TEXT,
        );
        p.text(
            egui::pos2(inner.right(), inner.top() - 2.0),
            egui::Align2::RIGHT_TOP,
            "REMAIN",
            egui::FontId::proportional(9.0),
            theme::TEXT_DIM,
        );
        let remain = (s.duration_secs() - pos).max(0.0);
        // Warn in the last 30 s, like CDJs.
        let warn = s.loaded && s.playing && remain < 30.0;
        p.text(
            egui::pos2(inner.right(), inner.bottom() + 2.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("-{}", fmt_tenths(remain)),
            egui::FontId::monospace(15.0),
            if warn && blink {
                theme::DANGER
            } else {
                theme::TEXT_DIM
            },
        );
    });
}

/// Vertical pitch fader with its readout (pulling down speeds up, as on CDJs).
fn pitch_column(app: &mut DjApp, ui: &mut Ui, deck: DeckId, s: &DeckState) {
    let accent = theme::deck_colour(deck);
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(RichText::new("TEMPO").size(9.0).color(theme::TEXT_DIM));
        ui.label(RichText::new("−").size(11.0).color(theme::TEXT_DIM));
        let h = (ui.available_height() - 18.0).max(60.0);
        let t = (s.pitch / PITCH_RANGE + 1.0) / 2.0;
        let (resp, new) = widgets::fader(
            ui,
            Vec2::new(40.0, h),
            true,
            true,
            t,
            0.5,
            widgets::FaderStyle {
                accent,
                ticks: 9,
                centre_detent: true,
                fill: false,
            },
        );
        if let Some(t) = new {
            let pitch = (t * 2.0 - 1.0) * PITCH_RANGE;
            app.audio.send(Command::SetPitch { deck, pitch });
            app.ui_fader(FaderTarget::Pitch(deck), pitch);
        }
        resp.on_hover_text(format!(
            "Tempo ±{:.0}% · pull down to speed up · Shift for fine · {RESET_HINT}",
            PITCH_RANGE * 100.0
        ));
        ui.label(RichText::new("+").size(11.0).color(theme::TEXT_DIM));
    });
}

/// `m:ss.t` (or `h:mm:ss.t`), for the deck time screens.
fn fmt_tenths(secs: f64) -> String {
    let tenths = (secs.max(0.0) * 10.0).floor() as u64;
    format!("{}.{}", fmt_time((tenths / 10) as f64), tenths % 10)
}

const RESET_HINT: &str = "Ctrl+click or double-click to reset";

fn mixer_strip(app: &mut DjApp, ui: &mut Ui) {
    let state = app.audio.state;
    let m = state.mixer;
    let defaults = MixerState::default();
    let phones_ok = state.headphones_available();
    let meters = app.meters;
    let rect = card(ui, theme::TEXT_DIM, |ui| {
        let full = ui.available_rect_before_wrap();
        // Lay the strip out on a fixed grid, centred in the card.
        let content_w = 194.0;
        let x0 = full.left() + ((full.width() - content_w) / 2.0).max(0.0);
        let y0 = full.top();
        let at = |x: f32, y: f32, w: f32, h: f32| {
            Rect::from_min_size(egui::pos2(x0 + x, y0 + y), Vec2::new(w, h))
        };
        let place = |ui: &mut Ui, r: Rect, add: &mut dyn FnMut(&mut Ui)| {
            ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| add(ui));
        };
        let label = |ui: &Ui, centre_x: f32, y: f32, text: &str, colour: Color32| {
            ui.painter().text(
                egui::pos2(x0 + centre_x, y0 + y),
                egui::Align2::CENTER_TOP,
                text,
                egui::FontId::proportional(9.0),
                colour,
            );
        };

        // Channel faders with their meters (headphone cue is on the 1/2 keys and the
        // controller).
        // Everything fits inside the card's height (the same as the decks): the
        // crossfader sits at the bottom and the faders take the height above it, leaving
        // room for their labels.
        let top = 4.0;
        let xf_h = 28.0;
        let xf_top = full.height() - xf_h;
        let fader_h = (xf_top - top - 22.0).max(60.0);
        for (deck, fx, mx) in [(DeckId::A, 60.0, 50.0), (DeckId::B, 110.0, 148.0)] {
            let i = deck.index();
            widgets::meter(
                ui.painter(),
                at(mx, top + 2.0, 6.0, fader_h - 4.0),
                meters[i],
            );
            place(ui, at(fx, top, 34.0, fader_h), &mut |ui| {
                let (resp, new) = widgets::fader(
                    ui,
                    Vec2::new(34.0, fader_h),
                    true,
                    false,
                    m.channel_fader[i],
                    defaults.channel_fader[i],
                    widgets::FaderStyle {
                        accent: theme::deck_colour(deck),
                        ticks: 11,
                        centre_detent: false,
                        fill: true,
                    },
                );
                if let Some(v) = new {
                    app.audio.send(Command::SetChannelFader { deck, value: v });
                    app.ui_fader(FaderTarget::Channel(deck), v);
                }
                resp.on_hover_text(format!("Channel {} · {RESET_HINT}", deck.label()));
            });
            label(
                ui,
                fx + 17.0,
                top + fader_h + 3.0,
                deck.label(),
                theme::deck_colour(deck),
            );
        }

        // Master meter (left/right).
        widgets::meter(
            ui.painter(),
            at(176.0, top + 2.0, 5.0, fader_h - 4.0),
            meters[2],
        );
        widgets::meter(
            ui.painter(),
            at(185.0, top + 2.0, 5.0, fader_h - 4.0),
            meters[3],
        );
        label(ui, 183.0, top + fader_h + 3.0, "MASTER", theme::TEXT_DIM);

        // Master and headphone level knobs.
        let knobs = [
            (
                "LEVEL",
                top + 4.0,
                m.master_level,
                defaults.master_level,
                true,
                true,
            ),
            (
                "PHONES",
                top + 74.0,
                m.headphone_level,
                defaults.headphone_level,
                phones_ok,
                false,
            ),
        ];
        for (name, y, value, default, enabled, is_master) in knobs {
            place(ui, at(1.0, y, 38.0, 38.0), &mut |ui| {
                let (resp, new) = widgets::knob(ui, 38.0, value, default, theme::TEXT, enabled);
                if let Some(v) = new {
                    app.audio.send(if is_master {
                        Command::SetMasterLevel(v)
                    } else {
                        Command::SetHeadphoneLevel(v)
                    });
                }
                let what = if is_master {
                    "Master level"
                } else {
                    "Headphone level"
                };
                resp.on_hover_text(format!("{what} · drag up/down · {RESET_HINT}"))
                    .on_disabled_hover_text("Headphones need a 4-channel device");
            });
            label(ui, 20.0, y + 40.0, name, theme::TEXT_DIM);
        }

        // Crossfader.
        let xf_w = full.width() - 36.0;
        let xf = Rect::from_min_size(
            egui::pos2(full.center().x - xf_w / 2.0, y0 + xf_top),
            Vec2::new(xf_w, xf_h),
        );
        ui.painter().text(
            egui::pos2(xf.left() - 10.0, xf.center().y),
            egui::Align2::CENTER_CENTER,
            "A",
            egui::FontId::proportional(12.0),
            theme::DECK_A,
        );
        ui.painter().text(
            egui::pos2(xf.right() + 10.0, xf.center().y),
            egui::Align2::CENTER_CENTER,
            "B",
            egui::FontId::proportional(12.0),
            theme::DECK_B,
        );
        place(ui, xf, &mut |ui| {
            let (resp, new) = widgets::fader(
                ui,
                xf.size(),
                false,
                false,
                m.crossfader,
                defaults.crossfader,
                widgets::FaderStyle {
                    accent: theme::TEXT,
                    ticks: 11,
                    centre_detent: true,
                    fill: false,
                },
            );
            if let Some(x) = new {
                app.audio.send(Command::SetCrossfader(x));
                app.ui_fader(FaderTarget::Crossfader, x);
            }
            resp.on_hover_text(format!("Crossfader · {RESET_HINT}"));
        });
    });
    // Dropping a track on the mixer loads the deck on that side.
    let mid_x = rect.center().x;
    track_drop_target(app, ui, rect, |p| {
        if p.x < mid_x { DeckId::A } else { DeckId::B }
    });
}

// ---------------------------------------------------------------- sidebar

/// The sidebar is the set list; the library itself is the main track list.
fn sidebar(app: &mut DjApp, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        section_heading(ui, "SETS");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let plus = ui
                .add(
                    egui::Button::new(RichText::new("+").size(16.0).strong())
                        .min_size(Vec2::new(24.0, 22.0))
                        .corner_radius(6),
                )
                .on_hover_text("New set");
            if plus.clicked() {
                app.new_set_name.clear();
                app.new_set_dialog = true;
            }
        });
    });
    // Filter at the top of the list (once there is something to filter).
    if !app.sets.is_empty() {
        ui.add(
            egui::TextEdit::singleline(&mut app.set_filter)
                .hint_text("Search sets…")
                .margin(egui::vec2(8.0, 5.0))
                .desired_width(ui.available_width()),
        );
        ui.add_space(4.0);
    }
    let needle = app.set_filter.trim().to_lowercase();
    let sets: Vec<_> = app
        .sets
        .iter()
        .filter(|s| needle.is_empty() || s.name.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    let mut action: Option<SetAction> = None;
    egui::ScrollArea::vertical()
        .id_salt("sets")
        .auto_shrink([false, true])
        .max_height(ui.available_height().max(60.0))
        .show(ui, |ui| {
            if app.sets.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("No sets yet. Click + to start planning a mix.")
                        .small()
                        .color(theme::TEXT_DIM),
                );
            } else if sets.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("No sets match “{}”", app.set_filter.trim()))
                        .small()
                        .color(theme::TEXT_DIM),
                );
            }
            for set in &sets {
                if let Some((id, name)) = &mut app.renaming
                    && *id == set.id
                {
                    let resp = ui.add(
                        egui::TextEdit::singleline(name)
                            .desired_width(ui.available_width())
                            .margin(egui::vec2(8.0, 8.0)),
                    );
                    if resp.lost_focus() {
                        let commit = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        action = Some(if commit {
                            SetAction::Rename(set.id, name.clone())
                        } else {
                            SetAction::CancelRename
                        });
                    } else {
                        resp.request_focus();
                    }
                    continue;
                }
                let tracks = match set.track_count {
                    1 => "1 track".to_string(),
                    n => format!("{n} tracks"),
                };
                let subtitle = if set.track_count == 0 {
                    "Empty".to_string()
                } else {
                    format!("{tracks}  ·  {}", fmt_time(set.total_secs))
                };
                let resp = list_row(
                    ui,
                    app.view == View::Set(set.id),
                    &set.name,
                    Some(&subtitle),
                );
                if resp.double_clicked() {
                    action = Some(SetAction::StartRename(set.id, set.name.clone()));
                } else if resp.clicked() {
                    action = Some(SetAction::Open(set.id));
                }
                resp.on_hover_text("Double-click to rename · right-click for more")
                    .context_menu(|ui| {
                        if ui.button("Rename").clicked() {
                            action = Some(SetAction::StartRename(set.id, set.name.clone()));
                            ui.close();
                        }
                        if ui.button("Delete set").clicked() {
                            action = Some(SetAction::Delete(set.id));
                            ui.close();
                        }
                    });
            }
        });

    match action {
        Some(SetAction::Open(id)) => app.set_view(View::Set(id)),
        Some(SetAction::StartRename(id, name)) => app.renaming = Some((id, name)),
        Some(SetAction::Rename(id, name)) => {
            app.renaming = None;
            if !name.trim().is_empty() {
                app.with_library(|lib| lib.rename_set(id, &name));
            }
        }
        Some(SetAction::CancelRename) => app.renaming = None,
        Some(SetAction::Delete(id)) => {
            app.with_library(|lib| lib.delete_set(id));
            app.notify("Set deleted (its tracks stay in the library)");
        }
        None => {}
    }
}

enum SetAction {
    Open(djk_library::SetId),
    StartRename(djk_library::SetId, String),
    Rename(djk_library::SetId, String),
    CancelRename,
    Delete(djk_library::SetId),
}

/// Modal dialog for naming a new set. Enter or Create saves; Esc, Cancel or a click
/// outside closes it.
fn new_set_dialog(app: &mut DjApp, ctx: &egui::Context) {
    let mut create = false;
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::unique("new_set_dialog")).show(ctx, |ui| {
        ui.set_width(320.0);
        ui.label(RichText::new("New set").size(18.0).strong());
        ui.label(
            RichText::new("Give it a name, like “Friday warm-up”. You can rename it later.")
                .small()
                .color(theme::TEXT_DIM),
        );
        ui.add_space(10.0);
        let field = ui.add(
            egui::TextEdit::singleline(&mut app.new_set_name)
                .hint_text("Set name")
                .margin(egui::vec2(8.0, 6.0))
                .desired_width(f32::INFINITY),
        );
        // Focus the name field as soon as the dialog opens.
        if !field.has_focus() && !field.lost_focus() {
            field.request_focus();
        }
        let valid = !app.new_set_name.trim().is_empty();
        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && valid {
            create = true;
        }
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let save = egui::Button::new(RichText::new("Create").strong())
                .fill(theme::DECK_A.gamma_multiply(0.45))
                .min_size(Vec2::new(80.0, 28.0));
            if ui.add_enabled(valid, save).clicked() {
                create = true;
            }
            if ui
                .add(egui::Button::new("Cancel").min_size(Vec2::new(80.0, 28.0)))
                .clicked()
            {
                close = true;
            }
        });
    });
    if modal.should_close() {
        close = true;
    }
    if create {
        let name = std::mem::take(&mut app.new_set_name);
        let mut created = None;
        app.with_library(|lib| {
            created = Some(lib.create_set(name.trim())?);
            Ok(())
        });
        if let Some(id) = created {
            app.set_view(View::Set(id));
        }
        close = true;
    }
    if close {
        app.new_set_dialog = false;
        app.new_set_name.clear();
    }
}

/// Small spaced-out caps heading for a sidebar section.
fn section_heading(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(10.0)
            .strong()
            .extra_letter_spacing(1.2)
            .color(theme::TEXT_DIM),
    );
    ui.add_space(2.0);
}

/// One-line text, cut with "…" to fit `width`.
fn truncated(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    colour: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat::simple(font, colour),
    );
    job.wrap = egui::text::TextWrapping {
        max_width: width.max(10.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// A set row: playlist icon tile, title and optional subtitle, with hover and selected
/// states (selected rows get a tinted fill and an accent bar).
fn list_row(ui: &mut Ui, selected: bool, title: &str, subtitle: Option<&str>) -> egui::Response {
    let h = if subtitle.is_some() { 44.0 } else { 34.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    let p = ui.painter();
    let accent = theme::DECK_A;
    if selected {
        p.rect_filled(rect, 7, accent.gamma_multiply(0.14));
        p.rect_filled(
            Rect::from_min_size(rect.min + Vec2::new(0.0, 8.0), Vec2::new(3.0, h - 16.0)),
            2,
            accent,
        );
    } else if resp.hovered() {
        p.rect_filled(rect, 7, Color32::from_white_alpha(8));
    }

    // Icon tile.
    let tile = Rect::from_center_size(
        egui::pos2(rect.left() + 22.0, rect.center().y),
        Vec2::splat(24.0),
    );
    let ink = if selected { accent } else { theme::TEXT_DIM };
    p.rect_filled(
        tile,
        6,
        if selected {
            accent.gamma_multiply(0.2)
        } else {
            theme::RAISED
        },
    );
    let c = tile.center();
    let stroke = egui::Stroke::new(1.6, ink);
    // A playlist: three lines and a note head.
    for (dy, w) in [(-5.0, 8.0), (0.0, 8.0), (5.0, 4.0)] {
        p.line_segment(
            [c + Vec2::new(-7.0, dy), c + Vec2::new(-7.0 + w, dy)],
            stroke,
        );
    }
    p.line_segment([c + Vec2::new(5.0, -6.0), c + Vec2::new(5.0, 4.0)], stroke);
    p.circle_filled(c + Vec2::new(3.0, 5.0), 2.4, ink);

    let text_right = rect.right() - 10.0;

    let x = tile.right() + 10.0;
    let width = text_right - x;
    let title_colour = if selected {
        theme::TEXT
    } else {
        theme::TEXT.gamma_multiply(0.92)
    };
    let tg = truncated(
        ui,
        title,
        egui::FontId::proportional(14.0),
        title_colour,
        width,
    );
    match subtitle {
        Some(sub) => {
            let sg = truncated(
                ui,
                sub,
                egui::FontId::proportional(11.0),
                theme::TEXT_DIM,
                width,
            );
            let top = rect.center().y - (tg.size().y + sg.size().y + 2.0) / 2.0;
            let th = tg.size().y;
            p.galley(egui::pos2(x, top), tg, title_colour);
            p.galley(egui::pos2(x, top + th + 2.0), sg, theme::TEXT_DIM);
        }
        None => {
            let y = rect.center().y - tg.size().y / 2.0;
            p.galley(egui::pos2(x, y), tg, title_colour);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Right-hand panel with everything known about the selected track.
fn track_details(app: &mut DjApp, ui: &mut Ui) {
    let Some(row) = app.selected_row().cloned() else {
        return;
    };
    ui.add_space(10.0);
    // Close button on the left, clear of the scroll bar on the right edge.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if ui
            .add(egui::Button::new(RichText::new("×").size(16.0)).frame(false))
            .on_hover_text("Close (Esc)")
            .clicked()
        {
            app.selected = None;
        }
        section_heading(ui, "TRACK DETAILS");
    });
    ui.add_space(4.0);
    ui.add(egui::Label::new(RichText::new(&row.title).size(17.0).strong()).wrap());
    ui.label(
        RichText::new(if row.artist.is_empty() {
            "Unknown artist"
        } else {
            &row.artist
        })
        .color(theme::TEXT_DIM),
    );
    ui.add_space(6.0);
    if let Some((id, bins)) = &app.details_overview
        && *id == row.id
    {
        waveview::overview(
            ui,
            Vec2::new(ui.available_width(), 44.0),
            bins,
            None,
            None,
            theme::TEXT_DIM,
            false,
        );
    }
    let analysis = app
        .library
        .as_ref()
        .and_then(|l| l.analysis(row.id).ok().flatten());

    // BPM and key tiles.
    ui.add_space(6.0);
    let tile_w = ((ui.available_width() - 8.0) / 2.0).max(60.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let tile = |ui: &mut Ui, label: &str, big: String, small: String, colour: Color32| {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(tile_w, 56.0), Sense::hover());
            let p = ui.painter();
            let inner = widgets::screen(p, r);
            p.text(
                inner.left_top() + Vec2::new(0.0, -2.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(9.0),
                theme::TEXT_DIM,
            );
            p.text(
                egui::pos2(inner.left(), inner.bottom() + 3.0),
                egui::Align2::LEFT_BOTTOM,
                big,
                egui::FontId::monospace(20.0),
                colour,
            );
            p.text(
                egui::pos2(inner.right(), inner.bottom() + 1.0),
                egui::Align2::RIGHT_BOTTOM,
                small,
                egui::FontId::proportional(10.0),
                theme::TEXT_DIM,
            );
            resp
        };
        let pending = analysis.is_none();
        let bpm = analysis.as_ref().and_then(|a| a.bpm);
        tile(
            ui,
            "BPM",
            bpm.map(|b| format!("{}", Bpm(b)))
                .unwrap_or_else(|| if pending { "…" } else { "—" }.into()),
            String::new(),
            theme::TEXT,
        );
        let key = analysis.as_ref().and_then(|a| a.key);
        tile(
            ui,
            "KEY",
            key.map(|k| k.camelot())
                .unwrap_or_else(|| if pending { "…" } else { "—" }.into()),
            key.map(|k| k.standard()).unwrap_or_default(),
            if key.is_some() {
                theme::KEY
            } else {
                theme::TEXT_DIM
            },
        );
    });

    ui.add_space(8.0);
    egui::Grid::new("details")
        .num_columns(2)
        .spacing([14.0, 5.0])
        .show(ui, |ui| {
            let mut kv = |k: &str, v: String| {
                ui.label(RichText::new(k).color(theme::TEXT_DIM));
                ui.label(v);
                ui.end_row();
            };
            match &analysis {
                Some(a) => {
                    kv("Duration", fmt_time(a.duration_secs));
                    kv("Format", a.format.clone());
                    kv(
                        "Sample rate",
                        format!("{:.1} kHz", f64::from(a.sample_rate) / 1000.0),
                    );
                    kv(
                        "Channels",
                        match a.channels {
                            1 => "Mono".into(),
                            2 => "Stereo".into(),
                            n => n.to_string(),
                        },
                    );
                    kv(
                        "Bitrate",
                        if a.lossless {
                            "Lossless".into()
                        } else {
                            a.bitrate_kbps
                                .map(|b| format!("{b} kbps"))
                                .unwrap_or_else(|| "—".into())
                        },
                    );
                }
                None => kv("Analysis", "In progress…".into()),
            }
        });
    if row.missing {
        ui.add_space(6.0);
        ui.colored_label(theme::DANGER, "File is missing");
    }
    ui.add_space(10.0);
    section_heading(ui, "LOCATION");
    ui.add(
        egui::Label::new(
            RichText::new(row.path.display().to_string())
                .small()
                .color(theme::TEXT_DIM),
        )
        .wrap(),
    );
    ui.add_space(10.0);
    ui.label(
        RichText::new("Drag the track onto a deck to load it, or press Shift+Left / Shift+Right.")
            .small()
            .color(theme::TEXT_DIM),
    );
}

// ---------------------------------------------------------------- browser

fn browser(app: &mut DjApp, ui: &mut Ui) {
    // Toolbar.
    ui.horizontal(|ui| {
        // Title, as a breadcrumb when a set is open: "Library / Set name", where
        // clicking "Library" goes back to the full library.
        match app.view {
            View::Library => {
                ui.heading("Library");
            }
            View::Set(id) => {
                let back = ui
                    .add(
                        egui::Label::new(
                            RichText::new("Library").size(18.0).color(theme::TEXT_DIM),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_text("Back to the full library");
                if back.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    ui.painter().line_segment(
                        [back.rect.left_bottom(), back.rect.right_bottom()],
                        egui::Stroke::new(1.0, theme::TEXT_DIM),
                    );
                }
                if back.clicked() {
                    app.set_view(View::Library);
                }
                ui.label(RichText::new("/").size(18.0).color(theme::TEXT_DIM));
                let name = app
                    .sets
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                ui.heading(name);
            }
        }
        ui.add_space(12.0);
        // Library view searches the library; an open set gets its own filter.
        let (text, hint) = match app.view {
            View::Library => (&mut app.query.search, "Search library  (Ctrl+F)"),
            View::Set(_) => (&mut app.set_track_filter, "Filter this set  (Ctrl+F)"),
        };
        let search = ui.add(
            egui::TextEdit::singleline(text)
                .hint_text(hint)
                .margin(egui::vec2(8.0, 4.0))
                .desired_width(240.0),
        );
        if app.focus_search {
            search.request_focus();
            app.focus_search = false;
        }
        if search.changed() {
            app.rows_dirty = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            match &app.import {
                Some(job) => {
                    let n = job.progress.load(std::sync::atomic::Ordering::Relaxed);
                    ui.spinner();
                    ui.label(format!("Importing… {n} files scanned"));
                }
                None => {
                    if ui.button("Import folder…").clicked()
                        && let Some(folder) = rfd::FileDialog::new().pick_folder()
                    {
                        let ctx = ui.ctx().clone();
                        app.start_import(vec![folder], None, &ctx);
                    }
                    if ui.button("Import files…").clicked()
                        && let Some(files) = rfd::FileDialog::new()
                            .add_filter("Audio", djk_analysis::decode::SUPPORTED_EXTENSIONS)
                            .pick_files()
                        && !files.is_empty()
                    {
                        let ctx = ui.ctx().clone();
                        app.start_import(files, None, &ctx);
                    }
                }
            }
            if let Some(pool) = &app.pool
                && pool.is_busy()
            {
                let (done, total) = pool.progress();
                ui.add(
                    egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                        .desired_width(140.0)
                        .text(format!("Analysing {done}/{total}")),
                );
            }
        });
    });
    ui.add_space(4.0);

    if app.rows.is_empty() {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            let filter = match app.view {
                View::Library => &app.query.search,
                View::Set(_) => &app.set_track_filter,
            };
            let msg = match (app.view, filter.trim().is_empty()) {
                (_, false) => "No tracks match your search.",
                (View::Library, true) => {
                    "Your library is empty. Import a music folder to get started."
                }
                (View::Set(_), true) => {
                    "This set is empty. Right-click tracks in the library to add them."
                }
            };
            ui.label(RichText::new(msg).color(theme::TEXT_DIM));
        });
        return;
    }

    track_table(app, ui);
}

#[derive(Clone, Copy)]
enum RowAction {
    Select(usize),
    Load(DeckId, usize),
    AddToSet(djk_library::SetId, usize),
    RemoveFromSet(usize),
    Move(usize, usize),
}

fn track_table(app: &mut DjApp, ui: &mut Ui) {
    let in_set = matches!(app.view, View::Set(_));
    // Row indices only match set positions when no search filter is hiding rows.
    let can_reorder = in_set && app.set_track_filter.trim().is_empty();
    let row_h = 26.0;
    let mut action: Option<RowAction> = None;
    let sort = app.query.sort;
    let desc = app.query.descending;
    let mut clicked_sort: Option<SortKey> = None;
    let sets = app.sets.clone();
    let rows = &app.rows;
    let selected = app.selected;

    // Resizable tables remember their column widths, which would stop the remainder
    // columns from stretching when the panel grows. Keying the stored state on the width
    // re-fits the columns whenever it changes (manual resizes last until then).
    let width_key = ui.available_width().round() as i32;
    let mut table = TableBuilder::new(ui)
        .id_salt(("track_table", width_key, in_set))
        .striped(true)
        .resizable(true)
        // Always use the full width and height, even with few or short rows.
        .auto_shrink(false)
        .sense(Sense::click_and_drag())
        .cell_layout(Layout::left_to_right(Align::Center))
        // The outer columns are fixed, so no resize line is drawn at either edge.
        .column(Column::exact(if in_set { 30.0 } else { 0.0 }).resizable(false))
        // Title and artist share whatever width is left, so the table always fills the
        // panel as the window or sidebar changes size.
        .column(Column::remainder().at_least(160.0).clip(true))
        .column(Column::remainder().at_least(80.0).clip(true))
        .column(Column::exact(60.0))
        .column(Column::exact(50.0))
        .column(Column::exact(56.0).resizable(false));
    if app.scroll_to_selected
        && let Some(i) = selected
    {
        table = table.scroll_to_row(i, None);
    }
    app.scroll_to_selected = false;

    let header_label =
        |ui: &mut Ui, text: &str, key: Option<SortKey>, clicked: &mut Option<SortKey>| {
            let arrow = match key {
                Some(k) if k == sort && !in_set => {
                    if desc {
                        " (desc)"
                    } else {
                        " (asc)"
                    }
                }
                _ => "",
            };
            let resp = ui.add(
                egui::Label::new(
                    RichText::new(format!("{text}{arrow}"))
                        .strong()
                        .color(theme::TEXT_DIM),
                )
                .sense(if key.is_some() && !in_set {
                    Sense::click()
                } else {
                    Sense::hover()
                }),
            );
            if resp.clicked() {
                *clicked = key;
            }
        };

    table
        .header(24.0, |mut header| {
            header.col(|ui| {
                if in_set {
                    header_label(ui, "#", None, &mut clicked_sort);
                }
            });
            header.col(|ui| header_label(ui, "Title", Some(SortKey::Title), &mut clicked_sort));
            header.col(|ui| header_label(ui, "Artist", Some(SortKey::Artist), &mut clicked_sort));
            header.col(|ui| header_label(ui, "BPM", Some(SortKey::Bpm), &mut clicked_sort));
            header.col(|ui| header_label(ui, "Key", Some(SortKey::Key), &mut clicked_sort));
            header.col(|ui| header_label(ui, "Time", Some(SortKey::Duration), &mut clicked_sort));
        })
        .body(|body| {
            body.rows(row_h, rows.len(), |mut row| {
                let i = row.index();
                let t = &rows[i];
                row.set_selected(selected == Some(i));
                let dim = |text: RichText| {
                    if t.missing {
                        text.strikethrough().color(theme::DANGER)
                    } else {
                        text
                    }
                };
                row.col(|ui| {
                    if in_set {
                        ui.label(RichText::new(format!("{}", i + 1)).color(theme::TEXT_DIM));
                    }
                });
                row.col(|ui| {
                    ui.add(
                        egui::Label::new(dim(RichText::new(&t.title)))
                            .truncate()
                            .selectable(false),
                    );
                    if t.missing {
                        ui.label(RichText::new("missing").small().color(theme::DANGER));
                    }
                });
                row.col(|ui| {
                    ui.add(
                        egui::Label::new(dim(RichText::new(&t.artist).color(theme::TEXT_DIM)))
                            .truncate()
                            .selectable(false),
                    );
                });
                row.col(|ui| {
                    let text = match (t.bpm, t.analysed) {
                        (Some(b), _) => format!("{b:.1}"),
                        (None, true) => "?".into(),
                        (None, false) => "…".into(),
                    };
                    ui.label(RichText::new(text).monospace());
                });
                row.col(|ui| {
                    let text = match (t.key, t.analysed) {
                        (Some(k), _) => k.camelot(),
                        (None, true) => "?".into(),
                        (None, false) => "…".into(),
                    };
                    let resp = ui.label(RichText::new(text).monospace());
                    if let Some(k) = t.key {
                        resp.on_hover_text(k.standard());
                    }
                });
                row.col(|ui| {
                    ui.label(
                        RichText::new(t.duration_secs.map(fmt_time).unwrap_or_else(|| "…".into()))
                            .monospace()
                            .color(theme::TEXT_DIM),
                    );
                });
                let resp = row.response();
                if resp.clicked() && action.is_none() {
                    action = Some(RowAction::Select(i));
                }
                // Rows can be dragged onto a deck (to load) and, in a set, between rows
                // (to reorder). Missing files can still be reordered, not loaded.
                if !t.missing || can_reorder {
                    resp.dnd_set_drag_payload(TrackDrag {
                        id: t.id,
                        title: t.title.clone(),
                        from_row: can_reorder.then_some(i),
                        loadable: !t.missing,
                    });
                }
                if can_reorder
                    && let Some(from) =
                        egui::DragAndDrop::payload::<TrackDrag>(&resp.ctx).and_then(|d| d.from_row)
                    && let Some(pos) = resp.ctx.pointer_latest_pos()
                    && resp.rect.contains(pos)
                {
                    // Insert above or below this row depending on which half is hovered.
                    let gap = if pos.y < resp.rect.center().y {
                        i
                    } else {
                        i + 1
                    };
                    let y = if gap == i {
                        resp.rect.top()
                    } else {
                        resp.rect.bottom()
                    };
                    let painter = resp.ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::unique("set_reorder_marker"),
                    ));
                    let x0 = resp.rect.left();
                    let x1 = resp.rect.right();
                    painter.line_segment(
                        [egui::pos2(x0, y), egui::pos2(x1, y)],
                        egui::Stroke::new(2.0, theme::DECK_A),
                    );
                    painter.circle_filled(egui::pos2(x0 + 3.0, y), 3.5, theme::DECK_A);
                    if resp.ctx.input(|inp| inp.pointer.any_released()) {
                        egui::DragAndDrop::clear_payload(&resp.ctx);
                        // Index after the dragged row is taken out.
                        let to = if gap > from { gap - 1 } else { gap };
                        if to != from {
                            action = Some(RowAction::Move(from, to));
                        }
                    }
                }
                resp.context_menu(|ui| {
                    for (deck, keys) in [(DeckId::A, "Shift+Left"), (DeckId::B, "Shift+Right")] {
                        if ui
                            .add_enabled(
                                !t.missing,
                                egui::Button::new(format!("Load to deck {}", deck.label()))
                                    .shortcut_text(keys),
                            )
                            .clicked()
                        {
                            action = Some(RowAction::Load(deck, i));
                            ui.close();
                        }
                    }
                    ui.separator();
                    ui.menu_button("Add to set", |ui| {
                        if sets.is_empty() {
                            ui.label(
                                RichText::new("Create a set in the sidebar first")
                                    .color(theme::TEXT_DIM),
                            );
                        }
                        for s in &sets {
                            if ui.button(&s.name).clicked() {
                                action = Some(RowAction::AddToSet(s.id, i));
                                ui.close();
                            }
                        }
                    });
                    if in_set && ui.button("Remove from set").clicked() {
                        action = Some(RowAction::RemoveFromSet(i));
                        ui.close();
                    }
                });
            });
        });

    // While a track is being dragged, show its title next to the pointer.
    if let Some(drag) = egui::DragAndDrop::payload::<TrackDrag>(ui.ctx())
        && let Some(pos) = ui.ctx().pointer_latest_pos()
    {
        egui::Area::new(egui::Id::unique("track_drag_ghost"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos + Vec2::new(14.0, 10.0))
            .interactable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new(&drag.title).strong());
                    let hint = match (drag.loadable, drag.from_row.is_some()) {
                        (true, true) => "Drop on a deck to load, or between rows to reorder",
                        (true, false) => "Drop on a deck to load",
                        (false, _) => "Drop between rows to reorder",
                    };
                    ui.label(RichText::new(hint).small().color(theme::TEXT_DIM));
                });
            });
    }

    if let Some(key) = clicked_sort {
        app.set_sort(key);
    }
    match action {
        Some(RowAction::Select(i)) => app.selected = Some(i),
        Some(RowAction::Load(deck, i)) => {
            app.selected = Some(i);
            let id = app.rows[i].id;
            app.load_track(deck, id);
        }
        Some(RowAction::AddToSet(set, i)) => {
            let id = app.rows[i].id;
            let name = app
                .sets
                .iter()
                .find(|s| s.id == set)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            let mut added = false;
            app.with_library(|lib| {
                added = lib.add_to_set(set, id)?;
                Ok(())
            });
            app.notify(if added {
                format!("Added to “{name}”")
            } else {
                format!("Already in “{name}”")
            });
        }
        Some(RowAction::RemoveFromSet(i)) => {
            if let View::Set(set) = app.view {
                let id = app.rows[i].id;
                app.with_library(|lib| lib.remove_from_set(set, id));
            }
        }
        Some(RowAction::Move(from, to)) => {
            if let View::Set(set) = app.view {
                app.with_library(|lib| lib.move_in_set(set, from, to));
                // The reload keeps the selection by track id, looked up from the old
                // order, so pointing at `from` keeps the moved track selected.
                app.selected = Some(from);
            }
        }
        None => {}
    }
}

// ---------------------------------------------------------------- status bar

fn status_bar(app: &mut DjApp, ui: &mut Ui) {
    let s = app.audio.state;
    let mut reconnect = false;
    let mut pick: Option<Option<String>> = None;
    ui.horizontal(|ui| {
        // Audio device picker.
        let current = app.audio.output.as_ref().map(|o| o.device_name.clone());
        let lost = app.audio.is_disconnected();
        let label = match (&current, lost) {
            (Some(name), false) => name.clone(),
            (Some(_), true) => "Audio device lost".into(),
            (None, _) => "No audio device".into(),
        };
        let colour = if lost { theme::DANGER } else { theme::TEXT };
        let combo = egui::ComboBox::from_id_salt("device")
            .selected_text(RichText::new(label).color(colour))
            .width(260.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(
                        app.settings.audio_device.is_none(),
                        "Automatic (prefer DDJ-FLX2)",
                    )
                    .clicked()
                {
                    pick = Some(None);
                }
                for name in &app.output_devices {
                    if ui
                        .selectable_label(current.as_deref() == Some(name), name)
                        .clicked()
                    {
                        pick = Some(Some(name.clone()));
                    }
                }
            });
        if combo.response.clicked() {
            app.refresh_devices();
        }
        if lost && ui.button("Reconnect").clicked() {
            reconnect = true;
        }
        if let Some(out) = &app.audio.output {
            ui.label(
                RichText::new(format!("{} ch · {} Hz", out.channels, out.sample_rate))
                    .color(theme::TEXT_DIM),
            );
        }
        ui.separator();
        let latency_ms = s.output_latency * 1000.0;
        ui.label(RichText::new(format!("Latency {latency_ms:.0} ms")).color(
            if latency_ms <= 20.0 {
                theme::TEXT_DIM
            } else {
                theme::WARN
            },
        ))
        .on_hover_text("Time from a button press to hearing it (buffer + device)");
        ui.label(
            RichText::new(format!("Overruns {}", s.overruns)).color(if s.overruns == 0 {
                theme::TEXT_DIM
            } else {
                theme::WARN
            }),
        );
        ui.separator();
        match app.controller.as_ref().and_then(|c| c.connected_port()) {
            Some(port) => ui.label(RichText::new(format!("Controller: {port}")).color(theme::OK)),
            None => ui.label(RichText::new("No controller").color(theme::TEXT_DIM)),
        };

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("Shortcuts").clicked() {
                app.show_shortcuts = !app.show_shortcuts;
            }
            if let Some((msg, t)) = &app.notice {
                if t.elapsed() < Duration::from_secs(8) {
                    ui.label(RichText::new(msg).color(theme::TEXT));
                } else {
                    app.notice = None;
                }
            }
        });
    });
    if reconnect {
        let name = app.settings.audio_device.clone();
        app.audio.switch_device(name.as_deref());
    }
    if let Some(choice) = pick {
        app.select_device(choice);
    }
}

fn shortcuts_window(app: &mut DjApp, ctx: &egui::Context) {
    let mut open = app.show_shortcuts;
    egui::Window::new("Keyboard shortcuts")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Grid::new("shortcuts")
                .num_columns(3)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Action").strong());
                    ui.label(RichText::new("Deck A").strong().color(theme::DECK_A));
                    ui.label(RichText::new("Deck B").strong().color(theme::DECK_B));
                    ui.end_row();
                    for (action, a, b) in crate::shortcuts::DECK_KEYS {
                        ui.label(*action);
                        ui.monospace(*a);
                        ui.monospace(*b);
                        ui.end_row();
                    }
                });
            ui.separator();
            for (keys, action) in crate::shortcuts::GLOBAL_KEYS {
                ui.horizontal(|ui| {
                    ui.monospace(*keys);
                    ui.label(*action);
                });
            }
        });
    app.show_shortcuts = open;
}

/// Full-window hint shown while files are dragged over the app.
pub fn drop_overlay(app: &DjApp, ctx: &egui::Context) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("drop-overlay"),
    ));
    let rect = ctx.content_rect().shrink(12.0);
    painter.rect_filled(rect, 12.0, Color32::from_black_alpha(190));
    painter.rect_stroke(
        rect,
        12.0,
        egui::Stroke::new(2.0, theme::DECK_A),
        egui::StrokeKind::Inside,
    );
    let target = match app.view {
        View::Set(id) => app
            .sets
            .iter()
            .find(|s| s.id == id)
            .map(|s| format!("Drop to import and add to “{}”", s.name)),
        View::Library => None,
    }
    .unwrap_or_else(|| "Drop to import into your library".to_owned());
    painter.text(
        rect.center() - egui::vec2(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        target,
        egui::FontId::proportional(24.0),
        theme::TEXT,
    );
    painter.text(
        rect.center() + egui::vec2(0.0, 20.0),
        egui::Align2::CENTER_CENTER,
        "Audio files or whole folders · MP3, FLAC, WAV, AIFF, M4A",
        egui::FontId::proportional(14.0),
        theme::TEXT_DIM,
    );
}
