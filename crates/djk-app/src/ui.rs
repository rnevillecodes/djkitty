//! Window layout and widgets: stacked full-width waveform lanes on top, then two decks
//! around a mixer strip, then the browser.

use std::time::{Duration, Instant};

use djk_controller::FaderTarget;
use djk_core::{
    Bpm, Command, DeckId, DeckState, EQ_NEUTRAL, EngineState, EqBand, HOT_CUE_COUNT, LOOP_BEATS,
    MixerState, PITCH_RANGE, PadMode,
};
use djk_library::SortKey;
use egui::{Align, Color32, Layout, Rect, RichText, Sense, Ui, Vec2};
use egui_extras::{Column, TableBuilder};

use crate::app::{DjApp, View};
use crate::theme;
use crate::util::fmt_time;
use crate::waveview;
use crate::widgets;

/// Height of the row holding the two deck panels and the mixer.
const DECKS_ROW_HEIGHT: f32 = 270.0;
const MIXER_WIDTH: f32 = 222.0;
/// Panel with the master and headphone level knobs, left of the mixer.
const LEVELS_WIDTH: f32 = 64.0;
/// Panel with the master meter, right of the mixer.
const MASTER_WIDTH: f32 = 58.0;
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
        .frame(egui::Frame::new().fill(theme::TOP_BG).inner_margin(8))
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
    if app.remove_prompt.is_some() {
        remove_dialog(app, ui.ctx());
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
        let grid = d.grid.filter(|_| s.loaded).map(|g| g.grid);
        let editing = d.grid_editing && grid.is_some();
        let lane = waveview::scrolling(
            ui,
            Vec2::new(width, lane_h),
            &waveview::Lane {
                peaks: peaks.as_deref(),
                bands: bands.as_deref(),
                band_norm: d.band_norm,
                position_secs: pos,
                cue_secs,
                grid,
                accent: theme::deck_colour(deck),
                label: deck.label(),
                editing,
                marks: waveview::Marks::from_deck(&s, |f| f / f64::from(s.track_rate.max(1))),
            },
            &mut app.wave_window_secs,
        );
        let rect = lane.response.rect;

        // Dragging the lane while editing slides the grid under the waveform.
        if let Some(g) = grid
            && editing
        {
            let dx = lane.response.drag_delta().x;
            if dx != 0.0 {
                app.set_deck_grid(deck, g.nudged(f64::from(dx) * lane.secs_per_point), false);
            }
            if lane.response.drag_stopped()
                && let Some(g) = app.decks[deck.index()].grid
            {
                app.set_deck_grid(deck, g.grid, true);
            }
        }
        let hint = if editing {
            "Drag to slide the beatgrid · scroll to zoom"
        } else {
            "Scroll to zoom"
        };
        lane.response
            .on_hover_text(format!("Showing {:.1} s · {hint}", app.wave_window_secs));
        if grid.is_some() {
            grid_tools(app, ui, deck, rect, pos, bands.as_deref());
        }
        track_drop_target(app, ui, rect, |_| deck);
    }
}

/// The GRID chip in a lane's top-right corner and, while editing, the beatgrid tools next
/// to it. Every edit pivots on the playhead (`pos`), so what you are looking at stays put.
fn grid_tools(
    app: &mut DjApp,
    ui: &Ui,
    deck: DeckId,
    lane: Rect,
    pos: f64,
    bands: Option<&djk_analysis::Waveform>,
) {
    let Some(stored) = app.decks[deck.index()].grid else {
        return;
    };
    let g = stored.grid;
    let editing = app.decks[deck.index()].grid_editing;
    let accent = theme::deck_colour(deck);
    egui::Area::new(egui::Id::unique(("grid_tools", deck.index())))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::RIGHT_TOP)
        .fixed_pos(lane.right_top() + Vec2::new(-6.0, 5.0))
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.spacing_mut().button_padding = egui::vec2(7.0, 2.0);
                let fine = ui.input(|i| i.modifiers.shift);
                if editing {
                    // Right to left is awkward with egui; build left to right instead.
                    if ui
                        .button("Set 1 here")
                        .on_hover_text(
                            "Make the beat at the playhead beat 1 of a bar \
                             (snaps to the nearest kick or transient)",
                        )
                        .clicked()
                    {
                        let t = bands.map_or(pos, |wf| waveview::snap_to_transient(wf, pos, 0.03));
                        app.set_deck_grid(deck, g.with_downbeat_at(t), true);
                    }
                    let step = if fine { 0.010 } else { 0.001 };
                    let nudge_hint = "Move the whole grid by 1 ms (Shift: 10 ms)";
                    if ui.button("Earlier").on_hover_text(nudge_hint).clicked() {
                        app.set_deck_grid(deck, g.nudged(-step), true);
                    }
                    if ui.button("Later").on_hover_text(nudge_hint).clicked() {
                        app.set_deck_grid(deck, g.nudged(step), true);
                    }
                    if ui.button("÷2").on_hover_text("Halve the tempo").clicked() {
                        app.set_deck_grid(deck, g.with_bpm(g.bpm / 2.0, pos), true);
                    }
                    if ui.button("×2").on_hover_text("Double the tempo").clicked() {
                        app.set_deck_grid(deck, g.with_bpm(g.bpm * 2.0, pos), true);
                    }
                    let mut bpm = g.bpm;
                    let resp = ui
                        .add(
                            egui::DragValue::new(&mut bpm)
                                .speed(0.01)
                                .range(20.0..=400.0)
                                .fixed_decimals(2)
                                .suffix(" BPM"),
                        )
                        .on_hover_text("Drag or type the grid's exact tempo");
                    if resp.changed() {
                        app.set_deck_grid(deck, g.with_bpm(bpm, pos), true);
                    }
                    if ui
                        .add_enabled(stored.edited, egui::Button::new("Reset"))
                        .on_hover_text("Go back to the detected beatgrid")
                        .on_disabled_hover_text("The grid is as detected")
                        .clicked()
                    {
                        app.reset_deck_grid(deck);
                    }
                }
                let chip = egui::Button::new(
                    RichText::new("GRID").size(11.0).strong().color(if editing {
                        theme::BG
                    } else {
                        theme::TEXT_DIM
                    }),
                )
                .fill(if editing { accent } else { theme::RAISED });
                let tip = if editing {
                    "Finish editing the beatgrid"
                } else if stored.edited {
                    "Edit the beatgrid (adjusted by you)"
                } else {
                    "Edit the beatgrid (as detected)"
                };
                if ui.add(chip).on_hover_text(tip).clicked() {
                    let d = &mut app.decks[deck.index()];
                    d.grid_editing = !d.grid_editing;
                }
            });
        });
}

fn decks_row(app: &mut DjApp, ui: &mut Ui) {
    let full = ui.available_size();
    use egui_extras::{Size, StripBuilder};
    if full.x < LEVELS_WIDTH + MIXER_WIDTH + MASTER_WIDTH + 400.0 {
        return; // window not laid out yet
    }
    // Deck A | levels | mixer | master meter | deck B
    StripBuilder::new(ui)
        .size(Size::remainder().at_least(200.0))
        .size(Size::exact(LEVELS_WIDTH))
        .size(Size::exact(MIXER_WIDTH))
        .size(Size::exact(MASTER_WIDTH))
        .size(Size::remainder().at_least(200.0))
        .horizontal(|mut strip| {
            strip.cell(|ui| deck_panel(app, ui, DeckId::A));
            strip.cell(|ui| levels_panel(app, ui));
            strip.cell(|ui| mixer_strip(app, ui));
            strip.cell(|ui| master_panel(app, ui));
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

/// A raised panel: lighter than the section behind it, with a soft drop shadow, a crisp
/// border and a thin accent strip along the top edge.
fn card(ui: &mut Ui, accent: Color32, add: impl FnOnce(&mut Ui)) -> Rect {
    let background = ui.painter().add(egui::Shape::Noop);
    let rect = egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, theme::CARD_BORDER))
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

    // Paint the background behind the content, now that the size is known: shadow,
    // panel, then the accent strip along the top edge.
    let shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(150),
    };
    let strip = Rect::from_min_size(
        rect.min + Vec2::new(14.0, 0.0),
        Vec2::new(rect.width() - 28.0, 2.0),
    );
    ui.painter().set(
        background,
        egui::Shape::Vec(vec![
            egui::Shape::Rect(shadow.as_shape(rect, 10)),
            egui::Shape::rect_filled(rect, 10, theme::CARD),
            egui::Shape::rect_filled(strip, 1, accent.gamma_multiply(0.85)),
        ]),
    );
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

/// Below this width the deck header switches to its compact form (narrower BPM screen,
/// shorter drag hint). At the default window size the decks are wider than this.
const DECK_NARROW_WIDTH: f32 = 300.0;

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

    // Header: deck badge, title/artist, and the BPM/key screen. Near the minimum window
    // size the screen narrows and the text truncates, so the header stays one row tall
    // and everything below it still fits in the card.
    let narrow = width < DECK_NARROW_WIDTH;
    let screen_w = if narrow { 92.0 } else { 132.0 };
    let title_size = if narrow { 14.0 } else { 16.0 };
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
                        egui::Label::new(RichText::new(&t.title).size(title_size).strong())
                            .truncate(),
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
                    ui.add(
                        egui::Label::new(
                            RichText::new("No track loaded")
                                .size(title_size)
                                .color(theme::TEXT_DIM),
                        )
                        .truncate(),
                    );
                    let hint = match (deck, narrow) {
                        (_, true) => "Drag a track here",
                        (DeckId::A, false) => {
                            "Drag a track here, or select one and press Shift+Left"
                        }
                        (DeckId::B, false) => {
                            "Drag a track here, or select one and press Shift+Right"
                        }
                    };
                    let hint = egui::Label::new(RichText::new(hint).small().color(theme::TEXT_DIM));
                    // Wide decks have room for the hint to wrap; narrow ones keep one line.
                    ui.add(if narrow { hint.truncate() } else { hint.wrap() });
                }
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(screen_w, 44.0), Sense::hover());
            let p = ui.painter();
            let inner = widgets::screen(p, r);
            // The narrow screen uses slightly smaller text so its readouts never overlap.
            let small_size = if narrow { 10.0 } else { 11.0 };
            let key = info.as_ref().and_then(|t| t.key);
            let bpm = info.as_ref().and_then(|t| t.bpm);
            let bpm_text = bpm
                .map(|b| format!("{}", Bpm(b).with_pitch(s.pitch)))
                .unwrap_or_else(|| "—".into());
            p.text(
                egui::pos2(inner.right(), inner.top() - 3.0),
                egui::Align2::RIGHT_TOP,
                bpm_text,
                egui::FontId::monospace(if narrow { 18.0 } else { 20.0 }),
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
                egui::FontId::proportional(small_size),
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
                egui::FontId::monospace(small_size),
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
        app.decks[deck.index()]
            .grid
            .filter(|_| s.loaded)
            .map(|g| (g.grid, s.duration_secs())),
        waveview::Marks::from_deck(s, |f| f / s.track_frames.max(1) as f64),
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

    // Transport buttons and the time screen (smaller buttons on a narrow deck, so the
    // screen keeps enough room for its readouts).
    let button_d = if narrow { 36.0 } else { 50.0 };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = if narrow { 4.0 } else { 6.0 };
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
            button_d,
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
            button_d,
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
        let grid = app.decks[deck.index()].grid.filter(|_| s.loaded);
        // "—" with no grid, or before the music (and so the grid) starts.
        let (bar_text, bar_colour) = match grid.and_then(|g| g.grid.bar_beat(pos)) {
            Some(bb) => {
                // Beat 1 lights up, so you can feel the bar going round.
                let colour = if bb.beat == 1 { accent } else { theme::TEXT };
                (bb.to_string(), colour)
            }
            None => ("—".to_string(), theme::TEXT_DIM),
        };
        let remain = (s.duration_secs() - pos).max(0.0);
        // Warn in the last 30 s, like CDJs.
        let warn = s.loaded && s.playing && remain < 30.0;
        let remain_colour = if warn && blink {
            theme::DANGER
        } else {
            theme::TEXT_DIM
        };
        if narrow {
            // Compact screen: elapsed on top, remaining and bar.beat below.
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, button_d + 8.0), Sense::hover());
            let p = ui.painter();
            let inner =
                widgets::screen(p, r.shrink2(Vec2::new(0.0, 1.0))).expand2(Vec2::new(3.0, 2.0));
            p.text(
                inner.left_top() + Vec2::new(0.0, -3.0),
                egui::Align2::LEFT_TOP,
                fmt_tenths(pos),
                egui::FontId::monospace(16.0),
                theme::TEXT,
            );
            p.text(
                inner.right_top() + Vec2::new(0.0, -2.0),
                egui::Align2::RIGHT_TOP,
                "BAR",
                egui::FontId::proportional(9.0),
                theme::TEXT_DIM,
            );
            p.text(
                egui::pos2(inner.left(), inner.bottom() + 2.0),
                egui::Align2::LEFT_BOTTOM,
                format!("-{}", fmt_tenths(remain)),
                egui::FontId::monospace(11.0),
                remain_colour,
            );
            p.text(
                egui::pos2(inner.right(), inner.bottom() + 2.0),
                egui::Align2::RIGHT_BOTTOM,
                bar_text,
                egui::FontId::monospace(12.0),
                bar_colour,
            );
            return;
        }
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
        // Bar and beat (e.g. 33.2), counted from bar 1 of the beatgrid.
        let cx = inner.center().x + 4.0;
        p.text(
            egui::pos2(cx, inner.top() - 2.0),
            egui::Align2::CENTER_TOP,
            "BAR",
            egui::FontId::proportional(9.0),
            theme::TEXT_DIM,
        );
        p.text(
            egui::pos2(cx, inner.bottom() + 2.0),
            egui::Align2::CENTER_BOTTOM,
            bar_text,
            egui::FontId::monospace(15.0),
            bar_colour,
        );
        p.text(
            egui::pos2(inner.right(), inner.bottom() + 2.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("-{}", fmt_tenths(remain)),
            egui::FontId::monospace(15.0),
            remain_colour,
        );
    });

    ui.add_space(4.0);
    pad_row(app, ui, deck, s);
}

/// Height of one performance pad. The pads sit in two rows, 1–4 over 5–8, like the
/// controller's pad grid.
const PAD_HEIGHT: f32 = 24.0;
/// Gap between the two rows of pads (and between the CUE and LOOP switch buttons).
const PAD_ROW_GAP: f32 = 2.0;

/// Short label for a loop length: 1/4, 1/2, 1, 2 … 32.
fn beats_label(beats: f64) -> String {
    if beats < 1.0 {
        format!("1/{}", (1.0 / beats).round())
    } else {
        format!("{beats}")
    }
}

/// The deck's eight performance pads, with a Hot Cue / Loop switch that mirrors the
/// controller's pad mode for this deck.
fn pad_row(app: &mut DjApp, ui: &mut Ui, deck: DeckId, s: &DeckState) {
    let mode = app.decks[deck.index()].pad_mode;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // Pads and the CUE/LOOP switch share one height; without this egui would pad the
        // switch buttons and the pad rows up to its own minimum interact height.
        ui.spacing_mut().interact_size.y = PAD_HEIGHT;
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = PAD_ROW_GAP;
            ui.spacing_mut().button_padding = Vec2::new(2.0, 0.0);
            for (m, label, tip) in [
                (PadMode::HotCue, "CUE", "Pads set and jump to hot cues"),
                (PadMode::BeatLoop, "LOOP", "Pads start beat loops"),
            ] {
                let on = mode == m;
                let b = ui
                    .add_sized(
                        [38.0, PAD_HEIGHT],
                        egui::Button::new(RichText::new(label).size(9.0).color(if on {
                            theme::TEXT
                        } else {
                            theme::TEXT_DIM
                        }))
                        .selected(on)
                        .corner_radius(3),
                    )
                    .on_hover_text(format!(
                        "{tip} (on the DDJ-FLX2: SHIFT + BEAT SYNC, then a pad)"
                    ));
                if b.clicked() {
                    app.set_pad_mode(deck, m);
                }
            }
        });
        let gap = ui.spacing().item_spacing.x;
        let per_row = HOT_CUE_COUNT / 2;
        let w = ((ui.available_width() - gap * (per_row - 1) as f32) / per_row as f32).max(14.0);
        let size = Vec2::new(w, PAD_HEIGHT);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = PAD_ROW_GAP;
            for row in 0..2 {
                ui.horizontal(|ui| {
                    for slot in (row * per_row..(row + 1) * per_row).map(|i| i as u8) {
                        match mode {
                            PadMode::HotCue => hot_cue_pad(app, ui, deck, s, slot, size),
                            PadMode::BeatLoop => loop_pad(app, ui, deck, s, slot, size),
                        }
                    }
                });
            }
        });
    });
}

fn hot_cue_pad(app: &mut DjApp, ui: &mut Ui, deck: DeckId, s: &DeckState, slot: u8, size: Vec2) {
    let cue = s.hot_cues[usize::from(slot)];
    let resp = widgets::pad(
        ui,
        size,
        &(slot + 1).to_string(),
        theme::HOT_CUE,
        cue.is_some(),
        s.loaded,
    );
    let (primary, shift) = ui.input(|i| (i.pointer.primary_down(), i.modifiers.shift));
    let down = resp.is_pointer_button_down_on() && primary;
    let held = &mut app.decks[deck.index()].pad_held;
    if down && held.is_none() {
        *held = Some(slot);
        app.audio.send(if shift && cue.is_some() {
            Command::ClearHotCue { deck, slot }
        } else {
            Command::HotCuePress { deck, slot }
        });
    } else if !down && *held == Some(slot) {
        *held = None;
        app.audio.send(Command::HotCueRelease { deck, slot });
    }
    if resp.secondary_clicked() && cue.is_some() {
        app.audio.send(Command::ClearHotCue { deck, slot });
    }
    let n = slot + 1;
    match cue {
        Some(frame) => {
            let at = fmt_tenths(frame / f64::from(s.track_rate.max(1)));
            resp.on_hover_text(format!(
                "Hot cue {n} at {at} · click to jump (hold to preview when paused) · \
                 right-click or Shift+click to clear"
            ));
        }
        None if s.loaded => {
            resp.on_hover_text(format!("Set hot cue {n} at the playhead"));
        }
        None => {}
    }
}

fn loop_pad(app: &mut DjApp, ui: &mut Ui, deck: DeckId, s: &DeckState, slot: u8, size: Vec2) {
    let beats = LOOP_BEATS[usize::from(slot)];
    let active = s.active_loop.is_some_and(|l| l.beats == beats);
    let enabled = s.loaded && s.beat_frames.is_some();
    let label = beats_label(beats);
    let resp = widgets::pad(ui, size, &label, theme::LOOP, active, enabled);
    if resp.clicked() {
        app.audio.send(Command::BeatLoop { deck, beats });
    }
    let unit = if beats > 1.0 { "beats" } else { "beat" };
    if !s.loaded {
    } else if !enabled {
        resp.on_hover_text("Needs a BPM to loop");
    } else if active {
        resp.on_hover_text(format!("Exit the {label}-{unit} loop"));
    } else if s.active_loop.is_some() {
        resp.on_hover_text(format!("Make the loop {label} {unit} long"));
    } else {
        resp.on_hover_text(format!("Loop {label} {unit} from the playhead"));
    }
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

/// Vertical layout shared by the mixer and the panels beside it, so their faders, meters
/// and knobs line up: everything fits the card's height (the same as the decks), with
/// the crossfader row at the bottom of the mixer and labels under the faders and meters.
struct StripLayout {
    /// Top of the faders/meters, relative to the card's content.
    top: f32,
    fader_h: f32,
    /// Top of the crossfader row.
    xf_top: f32,
    xf_h: f32,
}

impl StripLayout {
    fn new(content_height: f32) -> Self {
        let top = 4.0;
        let xf_h = 28.0;
        let xf_top = content_height - xf_h;
        Self {
            top,
            fader_h: (xf_top - top - 22.0).max(60.0),
            xf_top,
            xf_h,
        }
    }
}

/// Places widgets and labels on a fixed grid inside a card, `content_w` wide and centred.
struct Grid {
    x0: f32,
    y0: f32,
}

impl Grid {
    fn centred(full: Rect, content_w: f32) -> Self {
        Self {
            x0: full.left() + ((full.width() - content_w) / 2.0).max(0.0),
            y0: full.top(),
        }
    }

    fn at(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(egui::pos2(self.x0 + x, self.y0 + y), Vec2::new(w, h))
    }

    fn place(&self, ui: &mut Ui, r: Rect, add: impl FnOnce(&mut Ui)) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(r), add);
    }

    fn label(&self, ui: &Ui, centre_x: f32, y: f32, text: &str, colour: Color32) {
        ui.painter().text(
            egui::pos2(self.x0 + centre_x, self.y0 + y),
            egui::Align2::CENTER_TOP,
            text,
            egui::FontId::proportional(9.0),
            colour,
        );
    }
}

/// Master and headphone level knobs, in their own panel left of the mixer.
fn levels_panel(app: &mut DjApp, ui: &mut Ui) {
    let state = app.audio.state;
    let m = state.mixer;
    let defaults = MixerState::default();
    let phones_ok = state.headphones_available();
    card(ui, theme::TEXT_DIM, |ui| {
        let full = ui.available_rect_before_wrap();
        let lay = StripLayout::new(full.height());
        let d = 38.0;
        let g = Grid::centred(full, d);
        let knobs = [
            (
                "LEVEL",
                lay.top + 4.0,
                m.master_level,
                defaults.master_level,
                true,
                true,
            ),
            (
                "PHONES",
                lay.top + 74.0,
                m.headphone_level,
                defaults.headphone_level,
                phones_ok,
                false,
            ),
        ];
        for (name, y, value, default, enabled, is_master) in knobs {
            g.place(ui, g.at(0.0, y, d, d), |ui| {
                let (resp, new) = widgets::knob(ui, d, value, default, theme::TEXT, enabled, false);
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
            g.label(ui, d / 2.0, y + d + 2.0, name, theme::TEXT_DIM);
        }
    });
}

/// The master output meter (left and right), in its own panel right of the mixer.
fn master_panel(app: &mut DjApp, ui: &mut Ui) {
    let meters = app.meters;
    card(ui, theme::TEXT_DIM, |ui| {
        let full = ui.available_rect_before_wrap();
        let lay = StripLayout::new(full.height());
        let w = 16.0;
        let g = Grid::centred(full, w);
        let h = lay.fader_h - 4.0;
        widgets::meter(ui.painter(), g.at(0.0, lay.top + 2.0, 6.0, h), meters[2]);
        widgets::meter(ui.painter(), g.at(10.0, lay.top + 2.0, 6.0, h), meters[3]);
        g.label(
            ui,
            w / 2.0,
            lay.top + lay.fader_h + 3.0,
            "MASTER",
            theme::TEXT_DIM,
        );
    });
}

/// The mixer: per-channel EQ, meters and faders, with the crossfader underneath spanning
/// the same width.
fn mixer_strip(app: &mut DjApp, ui: &mut Ui) {
    let state = app.audio.state;
    let m = state.mixer;
    let defaults = MixerState::default();
    let meters = app.meters;
    let rect = card(ui, theme::TEXT_DIM, |ui| {
        let full = ui.available_rect_before_wrap();
        // Columns, left to right: deck A EQ, A meter, A fader, B fader, B meter, deck B EQ.
        let content_w = 170.0;
        let g = Grid::centred(full, content_w);
        let lay = StripLayout::new(full.height());
        let (top, fader_h) = (lay.top, lay.fader_h);

        // Channel faders with their meters (headphone cue is on the 1/2 keys and the
        // controller).
        for (deck, fx, mx) in [(DeckId::A, 48.0, 38.0), (DeckId::B, 92.0, 130.0)] {
            let i = deck.index();
            widgets::meter(
                ui.painter(),
                g.at(mx, top + 2.0, 6.0, fader_h - 4.0),
                meters[i],
            );
            g.place(ui, g.at(fx, top, 34.0, fader_h), |ui| {
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
            g.label(
                ui,
                fx + 17.0,
                top + fader_h + 3.0,
                deck.label(),
                theme::deck_colour(deck),
            );
        }

        // EQ: HI, MID and LOW knobs (top to bottom, as on a mixer) on the outer side of
        // each channel, each in its waveform band colour; red when the band is killed.
        let eq_d = 30.0;
        let eq_step = ((fader_h + 14.0) / 3.0).max(eq_d + 10.0);
        for (deck, cx) in [(DeckId::A, 15.0), (DeckId::B, 155.0)] {
            for (k, band) in EqBand::ALL.into_iter().enumerate() {
                let value = m.eq[deck.index()][band.index()];
                let colour = if value <= 0.001 {
                    theme::DANGER
                } else {
                    match band {
                        EqBand::High => theme::BAND_HIGH,
                        EqBand::Mid => theme::BAND_MID,
                        EqBand::Low => theme::BAND_LOW,
                    }
                };
                let y = top + k as f32 * eq_step;
                g.place(ui, g.at(cx - eq_d / 2.0, y, eq_d, eq_d), |ui| {
                    let (resp, new) =
                        widgets::knob(ui, eq_d, value, EQ_NEUTRAL, colour, true, true);
                    if let Some(v) = new {
                        app.audio.send(Command::SetEq {
                            deck,
                            band,
                            value: v,
                        });
                        app.ui_fader(FaderTarget::Eq(deck, band), v);
                    }
                    let shown = new.unwrap_or(value);
                    let db = match djk_audio::eq::knob_db(shown) {
                        None => "kill".to_string(),
                        Some(db) if db.abs() < 0.05 => "0 dB".to_string(),
                        Some(db) => format!("{db:+.1} dB"),
                    };
                    resp.on_hover_text(format!(
                        "Deck {} {} EQ: {db} · drag up/down · {RESET_HINT}",
                        deck.label(),
                        band.label()
                    ));
                });
                g.label(ui, cx, y + eq_d + 1.0, band.label(), theme::TEXT_DIM);
            }
        }

        // Crossfader: exactly as wide as the columns above, with A and B either side.
        let xf = g.at(0.0, lay.xf_top, content_w, lay.xf_h);
        for (x, text, colour) in [
            (xf.left() - 9.0, "A", theme::DECK_A),
            (xf.right() + 9.0, "B", theme::DECK_B),
        ] {
            ui.painter().text(
                egui::pos2(x, xf.center().y),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(12.0),
                colour,
            );
        }
        g.place(ui, xf, |ui| {
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
        // Line the heading up with the text in the search field below.
        ui.add_space(widgets::FIELD_TEXT_INSET);
        section_heading(ui, "SETS");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
            let plus = ui
                .add(
                    egui::Button::image_and_text(
                        widgets::icon(widgets::icons::PLUS, 11.0, theme::TEXT),
                        RichText::new("Add").size(11.0),
                    )
                    .min_size(Vec2::new(0.0, 20.0))
                    .corner_radius(5),
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
            widgets::text_field(&mut app.set_filter)
                .hint_text("Search sets…")
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
                        widgets::text_field(name)
                            .desired_width(ui.available_width())
                            .min_size(egui::vec2(0.0, 34.0)),
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
            widgets::text_field(&mut app.new_set_name)
                .hint_text("Set name")
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

/// "Remove from library?" confirmation. Makes clear the audio file is never deleted.
fn remove_dialog(app: &mut DjApp, ctx: &egui::Context) {
    let Some(prompt) = &app.remove_prompt else {
        return;
    };
    let (id, title, sets) = (prompt.id, prompt.title.clone(), prompt.sets);
    let mut remove = false;
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::unique("remove_dialog")).show(ctx, |ui| {
        ui.set_width(360.0);
        ui.label(RichText::new("Remove from library?").size(18.0).strong());
        ui.add_space(6.0);
        let where_from = match sets {
            0 => "your library".to_string(),
            1 => "your library and from 1 set".to_string(),
            n => format!("your library and from {n} sets"),
        };
        ui.label(format!("“{title}” will be removed from {where_from}."));
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "The audio file stays on your disk; nothing is deleted. \
                 Importing it again adds it back.",
            )
            .small()
            .color(theme::TEXT_DIM),
        );
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let confirm = egui::Button::new(RichText::new("Remove").strong())
                .fill(theme::DANGER.gamma_multiply(0.45))
                .min_size(Vec2::new(90.0, 28.0));
            if ui.add(confirm).clicked() {
                remove = true;
            }
            if ui
                .add(egui::Button::new("Cancel").min_size(Vec2::new(80.0, 28.0)))
                .clicked()
            {
                close = true;
            }
        });
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            remove = true;
        }
    });
    if modal.should_close() {
        close = true;
    }
    if remove {
        app.remove_prompt = None;
        app.remove_from_library(id);
    } else if close {
        app.remove_prompt = None;
    }
}

/// Spaced-out caps heading for a panel section (SETS, TRACK DETAILS, LOCATION).
fn section_heading(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(13.0)
            .strong()
            .extra_letter_spacing(1.4)
            .color(theme::HEADING),
    );
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
            None,
            waveview::Marks::default(),
        );
    }
    let analysis = app
        .library
        .as_ref()
        .and_then(|l| l.analysis(row.id).ok().flatten());

    // BPM, key and loudness tiles.
    ui.add_space(6.0);
    let tile_w = ((ui.available_width() - 16.0) / 3.0).max(60.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        // Label top left, a note (like the key's standard name) top right, value below.
        let tile = |ui: &mut Ui, label: &str, big: String, note: String, colour: Color32| {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(tile_w, 56.0), Sense::hover());
            let p = ui.painter();
            let inner = widgets::screen(p, r);
            p.text(
                inner.left_top() + Vec2::new(0.0, -2.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(9.0),
                theme::HEADING,
            );
            p.text(
                inner.right_top() + Vec2::new(0.0, -2.0),
                egui::Align2::RIGHT_TOP,
                note,
                egui::FontId::proportional(9.0),
                theme::TEXT_DIM,
            );
            p.text(
                egui::pos2(inner.left(), inner.bottom() + 3.0),
                egui::Align2::LEFT_BOTTOM,
                big,
                egui::FontId::monospace(18.0),
                colour,
            );
            resp
        };
        let pending = analysis.is_none();
        let unknown = || if pending { "…" } else { "—" }.to_string();
        let bpm = analysis.as_ref().and_then(|a| a.bpm);
        tile(
            ui,
            "BPM",
            bpm.map_or_else(unknown, |b| format!("{}", Bpm(b))),
            String::new(),
            theme::TEXT,
        );
        let key = analysis.as_ref().and_then(|a| a.key);
        tile(
            ui,
            "KEY",
            key.map_or_else(unknown, |k| k.camelot()),
            key.map(|k| k.standard()).unwrap_or_default(),
            if key.is_some() {
                theme::KEY
            } else {
                theme::TEXT_DIM
            },
        );
        let lufs = analysis.as_ref().and_then(|a| a.lufs);
        tile(
            ui,
            "LUFS",
            lufs.map_or_else(unknown, |l| format!("{l:.1}")),
            String::new(),
            theme::TEXT,
        )
        .on_hover_text(
            "Integrated loudness (EBU R128). Closer to 0 is louder; tracks a few LU \
             apart need the fader or trim evened out to blend.",
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
            widgets::text_field(text)
                .hint_text(hint)
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
                None => import_button(app, ui),
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
    RemoveFromLibrary(usize),
    Move(usize, usize),
}

fn track_table(app: &mut DjApp, ui: &mut Ui) {
    let in_set = matches!(app.view, View::Set(_));
    // Rows can be dragged to reorder only while the whole set shows in its own order.
    let can_reorder = in_set && app.set_track_filter.trim().is_empty() && app.set_in_own_order();
    let row_h = 26.0;
    let mut action: Option<RowAction> = None;
    let (sort, desc) = if in_set {
        (app.set_sort, app.set_sort_desc)
    } else {
        (Some(app.query.sort), app.query.descending)
    };
    let mut clicked_sort: Option<Option<SortKey>> = None;
    let sets = app.sets.clone();
    let rows = &app.rows;
    let row_numbers = &app.row_numbers;
    let selected = app.selected;

    // The columns always fill the list exactly: widths come from `app.track_columns`,
    // and dragging a divider (drawn after the table) moves width between neighbours.
    let area = ui.available_rect_before_wrap();
    // Room for the number and its sort arrow.
    let num_w = if in_set { 42.0 } else { 0.0 };
    let fit_total = track_columns_total(ui, area, num_w);
    let widths = app.track_columns.fit(fit_total);
    let mut table = TableBuilder::new(ui)
        .id_salt(("track_table", in_set))
        .striped(true)
        .resizable(false)
        // Always use the full width and height, even with few or short rows.
        .auto_shrink(false)
        .sense(Sense::click_and_drag())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(num_w));
    for &w in &widths {
        table = table.column(Column::exact(w).clip(true));
    }
    if app.scroll_to_selected
        && let Some(i) = selected
    {
        table = table.scroll_to_row(i, None);
    }
    app.scroll_to_selected = false;

    // Every header sorts: click once for ascending, again for descending. In a set, "#"
    // goes back to the set's own order.
    let header_label =
        |ui: &mut Ui, text: &str, key: Option<SortKey>, clicked: &mut Option<Option<SortKey>>| {
            let active = key == sort;
            let resp = ui
                .interact(
                    ui.max_rect(),
                    egui::Id::unique(("track_header", text)),
                    Sense::click(),
                )
                .on_hover_text(match (active, desc) {
                    (true, false) => "Sorted ascending; click to reverse",
                    (true, true) => "Sorted descending; click to reverse",
                    (false, _) => "Click to sort",
                });
            if resp.hovered() {
                ui.set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let colour = if active || resp.hovered() {
                theme::TEXT
            } else {
                theme::HEADING
            };
            ui.add(egui::Label::new(RichText::new(text).strong().color(colour)).selectable(false));
            if active {
                let arrow = if desc {
                    widgets::icons::CHEVRON_DOWN
                } else {
                    widgets::icons::CHEVRON_UP
                };
                ui.add(widgets::icon(arrow, 14.0, theme::DECK_A));
            }
            if resp.clicked() {
                *clicked = Some(key);
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
            header.col(|ui| header_label(ui, "LUFS", Some(SortKey::Lufs), &mut clicked_sort));
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
                        let n = row_numbers.get(i).copied().unwrap_or(i + 1);
                        ui.label(RichText::new(n.to_string()).color(theme::TEXT_DIM));
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
                    let text = match (t.lufs, t.analysed) {
                        (Some(l), _) => format!("{l:.1}"),
                        (None, true) => "?".into(),
                        (None, false) => "…".into(),
                    };
                    ui.label(RichText::new(text).monospace());
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
                    ui.separator();
                    // Neither option touches the audio file on disk.
                    if in_set
                        && ui
                            .add(egui::Button::new("Remove from set").shortcut_text("Del"))
                            .clicked()
                    {
                        action = Some(RowAction::RemoveFromSet(i));
                        ui.close();
                    }
                    let lib_remove = egui::Button::new("Remove from library…");
                    let lib_remove = if in_set {
                        lib_remove
                    } else {
                        lib_remove.shortcut_text("Del")
                    };
                    if ui
                        .add(lib_remove)
                        .on_hover_text("Takes it out of djkitty; the file stays on your disk")
                        .clicked()
                    {
                        action = Some(RowAction::RemoveFromLibrary(i));
                        ui.close();
                    }
                });
            });
        });

    column_dividers(app, ui, area, num_w, &widths, fit_total);

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
        Some(RowAction::RemoveFromSet(i)) => app.remove_from_open_set(i),
        Some(RowAction::RemoveFromLibrary(i)) => {
            let id = app.rows[i].id;
            app.ask_remove_from_library(id);
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

/// What to pick when importing.
#[derive(Clone, Copy)]
enum ImportPick {
    Files,
    Folder,
}

/// Split button: "Import" picks files; the arrow opens a menu to pick files or a folder.
/// Must be added inside a right-to-left layout (the arrow is added first, so it sits on
/// the right).
fn import_button(app: &mut DjApp, ui: &mut Ui) {
    let mut pick = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let height = widgets::FIELD_HEIGHT;
        let arrow = ui
            .add(
                egui::Button::image(widgets::icon(
                    widgets::icons::CHEVRON_DOWN,
                    16.0,
                    theme::TEXT,
                ))
                .min_size(Vec2::new(24.0, height))
                .corner_radius(egui::CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 6,
                    se: 6,
                }),
            )
            .on_hover_text("Import files or a folder");
        egui::Popup::menu(&arrow)
            .align(egui::RectAlign::BOTTOM_END)
            .show(|ui| {
                ui.set_min_width(140.0);
                if ui.add(menu_item("Files", widgets::icons::FILE)).clicked() {
                    pick = Some(ImportPick::Files);
                }
                if ui
                    .add(menu_item("Folder", widgets::icons::FOLDER))
                    .clicked()
                {
                    pick = Some(ImportPick::Folder);
                }
            });
        let main = ui
            .add(
                egui::Button::new("Import")
                    .min_size(Vec2::new(0.0, height))
                    .corner_radius(egui::CornerRadius {
                        nw: 6,
                        sw: 6,
                        ne: 0,
                        se: 0,
                    }),
            )
            .on_hover_text("Import audio files");
        if main.clicked() {
            pick = Some(ImportPick::Files);
        }
    });
    let paths = match pick {
        None => return,
        Some(ImportPick::Files) => rfd::FileDialog::new()
            .add_filter("Audio", djk_analysis::decode::SUPPORTED_EXTENSIONS)
            .pick_files()
            .unwrap_or_default(),
        Some(ImportPick::Folder) => rfd::FileDialog::new().pick_folder().into_iter().collect(),
    };
    if !paths.is_empty() {
        let ctx = ui.ctx().clone();
        app.start_import(paths, None, &ctx);
    }
}

// ---------------------------------------------------------------- status bar

fn status_bar(app: &mut DjApp, ui: &mut Ui) {
    let s = app.audio.state;
    let mut reconnect = false;
    let mut pick: Option<Option<String>> = None;
    ui.horizontal(|ui| {
        ui.add(egui::Image::new(widgets::icons::LOGO).fit_to_exact_size(Vec2::splat(18.0)))
            .on_hover_text("DJKitty");
        ui.label(
            RichText::new("DJKitty")
                .size(12.0)
                .strong()
                .color(theme::HEADING),
        );
        ui.separator();
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

/// A menu entry with its label on the left and an icon on the right.
fn menu_item(label: &str, icon: egui::ImageSource<'static>) -> egui::Button<'static> {
    egui::Button::new(label.to_owned()).right_text(widgets::icon(icon, 14.0, theme::TEXT_DIM))
}

// ---------------------------------------------------------------- track list columns

/// Default track list columns: (preferred width, minimum, grows with the list) for
/// title, artist, BPM, key, LUFS and time, with any saved widths applied.
pub fn track_columns(saved: Option<&[f32]>) -> crate::columns::FitColumns {
    let mut cols = crate::columns::FitColumns::new(&[
        (320.0, 120.0, true),
        (220.0, 80.0, true),
        (64.0, 48.0, false),
        (54.0, 42.0, false),
        (64.0, 50.0, false),
        (60.0, 50.0, false),
    ]);
    if let Some(saved) = saved {
        cols.set_preferred(saved);
    }
    cols
}

/// Width shared by the track list's resizable columns: the list's width less the "#"
/// column and the gaps between all seven columns (and a hair of slack for rounding).
fn track_columns_total(ui: &Ui, area: Rect, num_w: f32) -> f32 {
    area.width() - num_w - ui.spacing().item_spacing.x * 6.0 - 1.0
}

/// Draggable dividers between the track list's columns. Dragging moves width between
/// the two columns either side, so the columns keep filling the list exactly; the new
/// widths are saved when the drag ends.
fn column_dividers(
    app: &mut DjApp,
    ui: &mut Ui,
    area: Rect,
    num_w: f32,
    widths: &[f32],
    total: f32,
) {
    let gap = ui.spacing().item_spacing.x;
    // Dividers sit in the middle of the gap after each column (as egui's own do).
    let mut x = area.left() - gap * 0.5 + num_w + gap;
    for (i, w) in widths
        .iter()
        .enumerate()
        .take(widths.len().saturating_sub(1))
    {
        x += w + gap;
        let line = Rect::from_x_y_ranges(x..=x, area.y_range());
        let resp = ui.interact(
            line.expand2(Vec2::new(
                ui.style().interaction.resize_grab_radius_side,
                0.0,
            )),
            egui::Id::unique(("track_column_divider", i)),
            Sense::drag(),
        );
        if resp.dragged() {
            app.track_columns.drag(i, resp.drag_delta().x, total);
        }
        if resp.drag_stopped() {
            app.settings.track_columns = Some(app.track_columns.preferred().to_vec());
            app.settings.save();
        }
        let visuals = &ui.style().visuals.widgets;
        let stroke = if resp.dragged() {
            visuals.active.bg_stroke
        } else if resp.hovered() {
            visuals.hovered.bg_stroke
        } else {
            visuals.noninteractive.bg_stroke
        };
        if resp.hovered() || resp.dragged() {
            ui.set_cursor_icon(egui::CursorIcon::ResizeColumn);
        }
        ui.painter().vline(x, area.y_range(), stroke);
    }
}
