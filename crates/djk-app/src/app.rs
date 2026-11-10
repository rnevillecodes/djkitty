//! The main application window.

use djk_core::{Command, DeckId, LoadRequest};

use crate::audio::AudioService;
use crate::loader::{LoadJob, Loader};
use crate::util::fmt_time;

#[derive(Default)]
struct DeckUi {
    title: String,
    loading: bool,
    message: Option<String>,
    cue_held: bool,
}

pub struct DjApp {
    audio: AudioService,
    loader: Loader,
    decks: [DeckUi; 2],
    next_token: u64,
}

impl DjApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            audio: AudioService::start(None),
            loader: Loader::spawn(cc.egui_ctx.clone()),
            decks: Default::default(),
            next_token: 1,
        }
    }

    fn request_load(&mut self, deck: DeckId, path: std::path::PathBuf) {
        if self.audio.state.deck(deck).playing {
            self.decks[deck.index()].message = Some("Deck is playing — pause it first".into());
            return;
        }
        let token = self.next_token;
        self.next_token += 1;
        self.decks[deck.index()].loading = true;
        self.decks[deck.index()].message = None;
        self.loader.request(LoadJob { deck, path, token });
    }

    fn receive_loads(&mut self) {
        while let Some(done) = self.loader.try_recv() {
            let ui = &mut self.decks[done.deck.index()];
            ui.loading = false;
            match done.result {
                Ok(decoded) => {
                    if self.audio.state.deck(done.deck).playing {
                        ui.message = Some("Deck is playing — pause it first".into());
                        continue;
                    }
                    ui.title = done
                        .path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.audio.load(LoadRequest {
                        deck: done.deck,
                        buffer: decoded.buffer,
                        token: done.token,
                    });
                }
                Err(e) => ui.message = Some(format!("Could not load: {e}")),
            }
        }
    }

    fn deck_panel(&mut self, ui: &mut egui::Ui, deck: DeckId) {
        let s = *self.audio.state.deck(deck);
        ui.heading(format!("Deck {}", deck.label()));
        let d = &self.decks[deck.index()];
        if d.loading {
            ui.label("Loading…");
        } else if s.loaded {
            ui.label(&d.title);
        } else {
            ui.weak("No track loaded");
        }
        if let Some(msg) = &d.message {
            ui.colored_label(egui::Color32::from_rgb(240, 160, 60), msg);
        }
        ui.label(format!(
            "{}   -{}",
            fmt_time(s.position_secs()),
            fmt_time(s.remaining_secs())
        ));
        ui.horizontal(|ui| {
            if ui.button("Open…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Audio", djk_analysis::decode::SUPPORTED_EXTENSIONS)
                    .pick_file()
            {
                self.request_load(deck, path);
            }
            let play_label = if s.playing { "Pause" } else { "Play" };
            if ui.button(play_label).clicked() {
                self.audio.send(Command::TogglePlay(deck));
            }
            let cue = ui.button("Cue");
            let down = cue.is_pointer_button_down_on();
            let held = &mut self.decks[deck.index()].cue_held;
            if down && !*held {
                *held = true;
                self.audio.send(Command::CuePress(deck));
            } else if !down && *held {
                *held = false;
                self.audio.send(Command::CueRelease(deck));
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let s = self.audio.state;
        let mut reconnect = false;
        ui.horizontal(|ui| {
            match (&self.audio.output, &self.audio.error) {
                (Some(out), _) if !out.is_disconnected() => {
                    ui.label(format!(
                        "Audio: {} · {} ch @ {} Hz",
                        out.device_name, out.channels, out.sample_rate
                    ));
                }
                (Some(_), _) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, "Audio device lost");
                }
                (None, Some(err)) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("No audio: {err}"));
                }
                (None, None) => {
                    ui.label("No audio");
                }
            }
            if self.audio.is_disconnected() && ui.button("Reconnect").clicked() {
                reconnect = true;
            }
            ui.separator();
            let latency_ms = s.output_latency * 1000.0;
            let colour = if latency_ms <= 20.0 {
                ui.visuals().text_color()
            } else {
                egui::Color32::from_rgb(240, 160, 60)
            };
            ui.colored_label(colour, format!("Latency {latency_ms:.1} ms"));
            ui.separator();
            ui.label(format!("Overruns {}", s.overruns));
        });
        if reconnect {
            self.audio.switch_device(None);
        }
    }
}

impl eframe::App for DjApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.audio.poll();
        self.receive_loads();
        let playing = self.audio.state.decks.iter().any(|d| d.playing);
        if playing {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            ui.columns(2, |cols| {
                self.deck_panel(&mut cols[0], DeckId::A);
                self.deck_panel(&mut cols[1], DeckId::B);
            });
        });
    }
}
