//! Window layout and widgets: stacked full-width waveform lanes on top, then two decks
//! around a mixer strip, then the browser.

use std::time::{Duration, Instant};

use djk_controller::FaderTarget;
use djk_core::{Bpm, Command, DeckId, DeckState, EngineState, MixerState, PITCH_RANGE};
use djk_library::SortKey;
use egui::{Align, Color32, Layout, RichText, Sense, Ui, Vec2};
use egui_extras::{Column, TableBuilder};

use crate::app::{DjApp, View};
use crate::theme;
use crate::util::fmt_time;
use crate::waveview;

/// Height of the row holding the two deck panels and the mixer.
const DECKS_ROW_HEIGHT: f32 = 222.0;
const MIXER_WIDTH: f32 = 230.0;
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
    let lane_h = ((ui.available_height() - DECKS_ROW_HEIGHT - MIN_BROWSER_HEIGHT - 40.0) / 2.0)
        .clamp(80.0, 170.0);
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
        .default_size(240.0)
        .size_range(200.0..=360.0)
        .show(ui, |ui| sidebar(app, ui));
    egui::CentralPanel::default().show(ui, |ui| browser(app, ui));
    if app.show_shortcuts {
        shortcuts_window(app, ui.ctx());
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
        let wf = app.decks[deck.index()]
            .waveform
            .clone()
            .filter(|_| s.loaded);
        let cue_secs = s.loaded.then(|| s.cue / f64::from(s.track_rate.max(1)));
        waveview::scrolling(
            ui,
            Vec2::new(width, lane_h),
            wf.as_deref(),
            pos,
            cue_secs,
            theme::deck_colour(deck),
            deck.label(),
            &mut app.wave_window_secs,
        )
        .on_hover_text(format!(
            "Showing {:.1} s · scroll to zoom",
            app.wave_window_secs
        ));
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

fn card(ui: &mut Ui, accent: Color32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.25)))
        .corner_radius(8)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size().max(Vec2::ZERO));
            add(ui)
        });
}

fn deck_panel(app: &mut DjApp, ui: &mut Ui, deck: DeckId) {
    let accent = theme::deck_colour(deck);
    let state = app.audio.state;
    let s = *state.deck(deck);
    let now = Instant::now();
    card(ui, accent, |ui| {
        let width = ui.available_width();
        // Header: deck letter, title/artist, BPM/key.
        let info = app.decks[deck.index()]
            .track
            .as_ref()
            .filter(|_| s.loaded || app.decks[deck.index()].loading)
            .cloned();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(deck.label())
                    .size(22.0)
                    .strong()
                    .color(accent),
            );
            ui.vertical(|ui| {
                ui.set_width((width - 170.0).max(40.0));
                match &info {
                    Some(t) => {
                        ui.add(
                            egui::Label::new(RichText::new(&t.title).size(15.0).strong())
                                .truncate(),
                        );
                        let artist = if t.artist.is_empty() {
                            "—"
                        } else {
                            &t.artist
                        };
                        ui.add(
                            egui::Label::new(RichText::new(artist).color(theme::TEXT_DIM))
                                .truncate(),
                        );
                    }
                    None => {
                        ui.label(RichText::new("No track loaded").color(theme::TEXT_DIM));
                        ui.label(
                            RichText::new("Load from the library below")
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                    }
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let key = info.as_ref().and_then(|t| t.key);
                let bpm = info.as_ref().and_then(|t| t.bpm);
                ui.vertical(|ui| {
                    let bpm_text = bpm
                        .map(|b| format!("{} BPM", Bpm(b).with_pitch(s.pitch)))
                        .unwrap_or_else(|| "— BPM".into());
                    ui.label(RichText::new(bpm_text).size(20.0).strong().monospace());
                    ui.label(
                        RichText::new(
                            key.map(|k| k.camelot())
                                .map(|c| format!("Key {c}"))
                                .unwrap_or_else(|| "Key —".into()),
                        )
                        .color(theme::TEXT_DIM),
                    )
                    .on_hover_text(
                        key.map(|k| k.standard())
                            .unwrap_or_else(|| "Key unknown".into()),
                    );
                });
            });
        });

        // Status line: loading / messages.
        let d = &mut app.decks[deck.index()];
        if d.message
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(MESSAGE_SECS))
        {
            d.message = None;
        }
        if d.loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading…");
            });
        } else if let Some((msg, _)) = &d.message {
            ui.colored_label(theme::WARN, msg);
        }

        // Overview with click-to-seek while paused. (The scrolling waveform lives in the
        // stacked lanes above the decks.)
        let pos = display_position_secs(&state, &s, now);
        let dur = s.duration_secs().max(1e-6);
        let ov = waveview::overview(
            ui,
            Vec2::new(width, 44.0),
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

        // Time.
        ui.horizontal(|ui| {
            ui.label(RichText::new(fmt_time(pos)).size(18.0).monospace());
            ui.label(
                RichText::new(format!("-{}", fmt_time((s.duration_secs() - pos).max(0.0))))
                    .size(18.0)
                    .monospace()
                    .color(theme::TEXT_DIM),
            );
        });

        // Transport + pitch.
        ui.horizontal(|ui| {
            let big = Vec2::new(64.0, 40.0);
            let cue_btn = ui.add_sized(
                big,
                egui::Button::new(RichText::new("CUE").strong())
                    .fill(if s.previewing || (s.loaded && !s.playing && s.at_cue()) {
                        theme::CUE_MARKER.gamma_multiply(0.45)
                    } else {
                        theme::RAISED
                    })
                    .sense(Sense::click_and_drag()),
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

            let (label, fill) = if s.playing && !s.previewing {
                ("PAUSE", accent.gamma_multiply(0.6))
            } else {
                ("PLAY", theme::RAISED)
            };
            if ui
                .add_sized(
                    big,
                    egui::Button::new(RichText::new(label).size(18.0)).fill(fill),
                )
                .on_hover_text("Play / pause")
                .clicked()
            {
                app.audio.send(Command::TogglePlay(deck));
            }

            ui.add_space(8.0);
            ui.vertical(|ui| {
                let mut pitch_pct = s.pitch * 100.0;
                let range = PITCH_RANGE * 100.0;
                let resp = ui.add(
                    egui::Slider::new(&mut pitch_pct, -range..=range)
                        .step_by(0.05)
                        .show_value(false)
                        .text("Pitch"),
                );
                if let Some(pct) = slider_value(ui, &resp, pitch_pct, 0.0) {
                    let pitch = pct / 100.0;
                    app.audio.send(Command::SetPitch { deck, pitch });
                    app.ui_fader(FaderTarget::Pitch(deck), pitch);
                }
                resp.on_hover_text(RESET_HINT);
                ui.label(
                    RichText::new(format!("{:+.1}%", s.pitch * 100.0))
                        .monospace()
                        .color(if s.pitch.abs() < 1e-4 {
                            theme::TEXT_DIM
                        } else {
                            accent
                        }),
                );
            });
        });
    });
}

const RESET_HINT: &str = "Ctrl+click or double-click to reset";

/// Interpret a slider response: Ctrl+click (Cmd+click on macOS) or double-click resets to
/// `default`; otherwise a changed value is returned as is.
fn slider_value(ui: &Ui, resp: &egui::Response, value: f32, default: f32) -> Option<f32> {
    let reset_modifier = ui.input(|i| i.modifiers.command);
    let touched = resp.clicked() || resp.drag_started() || resp.changed();
    if (reset_modifier && touched) || resp.double_clicked() {
        Some(default)
    } else if resp.changed() {
        Some(value)
    } else {
        None
    }
}

fn mixer_strip(app: &mut DjApp, ui: &mut Ui) {
    let state = app.audio.state;
    let m = state.mixer;
    card(ui, theme::TEXT_DIM, |ui| {
        // Headphone cue toggles.
        ui.columns(2, |cols| {
            for (i, deck) in DeckId::ALL.into_iter().enumerate() {
                cols[i].vertical_centered(|ui| {
                    let on = m.headphone_cue[deck.index()];
                    let enabled = state.headphones_available();
                    let btn = egui::Button::new(RichText::new(format!("Phones {}", deck.label())))
                        .fill(if on {
                            theme::deck_colour(deck).gamma_multiply(0.55)
                        } else {
                            theme::RAISED
                        });
                    let resp = ui.add_enabled(enabled, btn);
                    if resp.clicked() {
                        app.audio.send(Command::ToggleHeadphoneCue(deck));
                    }
                    resp.on_hover_text("Headphone cue").on_disabled_hover_text(
                        "Headphone cue needs a 4-channel device (like the DDJ-FLX2)",
                    );
                });
            }
        });

        // Vertical faders side by side: channel A, channel B, master, headphones.
        ui.add_space(4.0);
        let defaults = MixerState::default();
        let phones_ok = state.headphones_available();
        ui.columns(4, |cols| {
            for (i, deck) in DeckId::ALL.into_iter().enumerate() {
                let v = m.channel_fader[deck.index()];
                let label = RichText::new(deck.label())
                    .color(theme::deck_colour(deck))
                    .strong();
                let default = defaults.channel_fader[deck.index()];
                if let Some(v) = vertical_fader(&mut cols[i], v, default, label, true) {
                    app.audio.send(Command::SetChannelFader { deck, value: v });
                    app.ui_fader(FaderTarget::Channel(deck), v);
                }
            }
            let label = RichText::new("Master").small().color(theme::TEXT_DIM);
            if let Some(v) =
                vertical_fader(&mut cols[2], m.master_level, defaults.master_level, label, true)
            {
                app.audio.send(Command::SetMasterLevel(v));
            }
            let label = RichText::new("Phones").small().color(theme::TEXT_DIM);
            if let Some(v) = vertical_fader(
                &mut cols[3],
                m.headphone_level,
                defaults.headphone_level,
                label,
                phones_ok,
            ) {
                app.audio.send(Command::SetHeadphoneLevel(v));
            }
        });

        // Crossfader.
        ui.add_space(2.0);
        ui.vertical_centered(|ui| {
            let mut x = m.crossfader;
            ui.spacing_mut().slider_width = MIXER_WIDTH - 40.0;
            let resp = ui.add(egui::Slider::new(&mut x, 0.0..=1.0).show_value(false));
            if let Some(x) = slider_value(ui, &resp, x, MixerState::default().crossfader) {
                app.audio.send(Command::SetCrossfader(x));
                app.ui_fader(FaderTarget::Crossfader, x);
            }
            resp.on_hover_text(format!("Crossfader · {RESET_HINT}"));
            ui.label(
                RichText::new("A  ·  crossfader  ·  B")
                    .small()
                    .color(theme::TEXT_DIM),
            );
        });

    });
}

/// One vertical 0–1 fader with a label underneath, centred in its column. Returns the new
/// value when the user moved or reset it.
fn vertical_fader(
    ui: &mut Ui,
    value: f32,
    default: f32,
    label: RichText,
    enabled: bool,
) -> Option<f32> {
    ui.vertical_centered(|ui| {
        let mut v = value;
        ui.spacing_mut().slider_width = 84.0;
        let resp = ui.add_enabled(
            enabled,
            egui::Slider::new(&mut v, 0.0..=1.0)
                .vertical()
                .show_value(false),
        );
        let out = slider_value(ui, &resp, v, default);
        resp.on_hover_text(RESET_HINT)
            .on_disabled_hover_text("Needs a 4-channel device (like the DDJ-FLX2)");
        ui.label(label);
        out
    })
    .inner
}

// ---------------------------------------------------------------- sidebar

fn sidebar(app: &mut DjApp, ui: &mut Ui) {
    ui.add_space(6.0);
    let lib_count = app
        .library
        .as_ref()
        .and_then(|l| l.track_count().ok())
        .unwrap_or(0);
    if ui
        .selectable_label(app.view == View::Library, format!("Library  ({lib_count})"))
        .clicked()
    {
        app.set_view(View::Library);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("SETS")
                .small()
                .strong()
                .color(theme::TEXT_DIM),
        );
    });

    let sets = app.sets.clone();
    let mut action: Option<SetAction> = None;
    egui::ScrollArea::vertical()
        .id_salt("sets")
        .max_height((ui.available_height() - 210.0).max(80.0))
        .show(ui, |ui| {
            for set in &sets {
                if let Some((id, name)) = &mut app.renaming
                    && *id == set.id
                {
                    let resp = ui.text_edit_singleline(name);
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
                let label = format!(
                    "{}  ·  {} · {}",
                    set.name,
                    set.track_count,
                    fmt_time(set.total_secs)
                );
                let resp = ui.selectable_label(app.view == View::Set(set.id), label);
                if resp.clicked() {
                    action = Some(SetAction::Open(set.id));
                }
                resp.context_menu(|ui| {
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
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut app.new_set_name)
                .hint_text("New set name")
                .desired_width((ui.available_width() - 64.0).max(60.0)),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button("Add").on_hover_text("Create set").clicked() || enter)
            && !app.new_set_name.trim().is_empty()
        {
            action = Some(SetAction::Create(std::mem::take(&mut app.new_set_name)));
        }
    });

    match action {
        Some(SetAction::Open(id)) => app.set_view(View::Set(id)),
        Some(SetAction::Create(name)) => {
            let mut created = None;
            app.with_library(|lib| {
                created = Some(lib.create_set(&name)?);
                Ok(())
            });
            if let Some(id) = created {
                app.set_view(View::Set(id));
            }
        }
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

    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt("details")
        .auto_shrink([false, false])
        .show(ui, |ui| track_details(app, ui));
}

enum SetAction {
    Open(djk_library::SetId),
    Create(String),
    StartRename(djk_library::SetId, String),
    Rename(djk_library::SetId, String),
    CancelRename,
    Delete(djk_library::SetId),
}

fn track_details(app: &mut DjApp, ui: &mut Ui) {
    ui.label(
        RichText::new("TRACK DETAILS")
            .small()
            .strong()
            .color(theme::TEXT_DIM),
    );
    let Some(row) = app.selected_row().cloned() else {
        ui.label(RichText::new("Select a track").color(theme::TEXT_DIM));
        return;
    };
    ui.add(egui::Label::new(RichText::new(&row.title).strong()).truncate());
    if !row.artist.is_empty() {
        ui.add(egui::Label::new(RichText::new(&row.artist).color(theme::TEXT_DIM)).truncate());
    }
    if let Some((id, bins)) = &app.details_overview
        && *id == row.id
    {
        waveview::overview(
            ui,
            Vec2::new(ui.available_width(), 30.0),
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
    egui::Grid::new("details")
        .num_columns(2)
        .spacing([10.0, 2.0])
        .show(ui, |ui| {
            let mut kv = |k: &str, v: String| {
                ui.label(RichText::new(k).color(theme::TEXT_DIM));
                ui.label(v);
                ui.end_row();
            };
            match &analysis {
                Some(a) => {
                    kv("Duration", fmt_time(a.duration_secs));
                    kv(
                        "BPM",
                        a.bpm
                            .map(|b| format!("{}", Bpm(b)))
                            .unwrap_or_else(|| "unknown".into()),
                    );
                    kv(
                        "Key",
                        a.key
                            .map(|k| format!("{}  ({})", k.camelot(), k.standard()))
                            .unwrap_or_else(|| "unknown".into()),
                    );
                    kv("Format", a.format.clone());
                    kv("Sample rate", format!("{} Hz", a.sample_rate));
                    kv("Channels", a.channels.to_string());
                    kv(
                        "Bitrate",
                        if a.lossless {
                            "lossless".into()
                        } else {
                            a.bitrate_kbps
                                .map(|b| format!("{b} kbps"))
                                .unwrap_or_else(|| "—".into())
                        },
                    );
                }
                None => kv("Analysis", "pending…".into()),
            }
        });
    if row.missing {
        ui.colored_label(theme::DANGER, "File is missing");
    }
    ui.add(
        egui::Label::new(
            RichText::new(row.path.display().to_string())
                .small()
                .color(theme::TEXT_DIM),
        )
        .truncate(),
    );
}

// ---------------------------------------------------------------- browser

fn browser(app: &mut DjApp, ui: &mut Ui) {
    // Toolbar.
    ui.horizontal(|ui| {
        let title = match app.view {
            View::Library => "Library".to_owned(),
            View::Set(id) => app
                .sets
                .iter()
                .find(|s| s.id == id)
                .map(|s| format!("Set: {}", s.name))
                .unwrap_or_default(),
        };
        ui.heading(title);
        ui.add_space(12.0);
        let search = ui.add(
            egui::TextEdit::singleline(&mut app.query.search)
                .hint_text("Search title or artist  (Ctrl+F)")
                .desired_width(260.0),
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
            let msg = match (app.view, app.query.search.is_empty()) {
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
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(if in_set { 30.0 } else { 0.0 }))
        // Title and artist share whatever width is left, so the table always fills the
        // panel as the window or sidebar changes size.
        .column(Column::remainder().at_least(160.0).clip(true))
        .column(Column::remainder().at_least(80.0).clip(true))
        .column(Column::exact(60.0))
        .column(Column::exact(50.0))
        .column(Column::exact(56.0))
        .column(Column::exact(if in_set { 150.0 } else { 80.0 }));
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
            header.col(|ui| header_label(ui, "Load", None, &mut clicked_sort));
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
                row.col(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(6.0, 1.0);
                    for deck in DeckId::ALL {
                        let b = egui::Button::new(
                            RichText::new(deck.label())
                                .color(theme::deck_colour(deck))
                                .strong(),
                        );
                        if ui
                            .add_enabled(!t.missing, b)
                            .on_hover_text(format!("Load to deck {}", deck.label()))
                            .clicked()
                        {
                            action = Some(RowAction::Load(deck, i));
                        }
                    }
                    if in_set {
                        if ui
                            .add_enabled(i > 0, egui::Button::new("Up"))
                            .on_hover_text("Move up")
                            .clicked()
                        {
                            action = Some(RowAction::Move(i, i - 1));
                        }
                        if ui
                            .add_enabled(i + 1 < rows.len(), egui::Button::new("Down"))
                            .on_hover_text("Move down")
                            .clicked()
                        {
                            action = Some(RowAction::Move(i, i + 1));
                        }
                    }
                });
                let resp = row.response();
                if resp.clicked() && action.is_none() {
                    action = Some(RowAction::Select(i));
                }
                resp.context_menu(|ui| {
                    for deck in DeckId::ALL {
                        if ui
                            .button(format!("Load to deck {}", deck.label()))
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
                app.selected = Some(to);
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
