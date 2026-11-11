//! The main application: state, background services and per-frame wiring.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use djk_analysis::Waveform;
use djk_analysis::waveform::OVERVIEW_BINS;
use djk_controller::{Action, ControllerManager, DDJ_FLX2};
use djk_core::{DeckId, LoadRequest, TrackId};
use djk_library::{ImportSummary, Library, SetId, SetSummary, SortKey, TrackQuery, TrackRow};

use crate::analysis::{AnalysisEvent, AnalysisPool};
use crate::audio::AudioService;
use crate::loader::{LoadJob, Loader};
use crate::settings::Settings;
use crate::waveview::{Peaks, WaveCol};

/// What a deck shows about its track.
#[derive(Default)]
pub struct DeckUi {
    /// Token of the track this info belongs to (matches the engine's `track_token`).
    pub token: u64,
    pub track: Option<TrackRow>,
    /// Band data, used to tint the waveform.
    pub waveform: Option<Arc<Waveform>>,
    /// Min/max summary of the audio, used for the waveform shape.
    pub peaks: Option<Arc<Peaks>>,
    pub overview: Vec<WaveCol>,
    pub loading: bool,
    pub message: Option<(String, Instant)>,
    pub cue_held: bool,
}

/// Which list the browser shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Library,
    Set(SetId),
}

/// Import running on a background thread.
pub struct ImportJob {
    pub progress: Arc<std::sync::atomic::AtomicUsize>,
    /// Summary, plus how many tracks were appended to the target set (if any).
    pub result: Receiver<Result<(ImportSummary, Option<usize>), String>>,
}

pub struct DjApp {
    pub audio: AudioService,
    pub loader: Loader,
    pub library: Option<Library>,
    pub data_dir: PathBuf,
    pub pool: Option<AnalysisPool>,
    pub controller: Option<ControllerManager>,
    pub settings: Settings,
    pub decks: [DeckUi; 2],

    pub view: View,
    pub query: TrackQuery,
    pub rows: Vec<TrackRow>,
    pub rows_dirty: bool,
    pub selected: Option<usize>,
    pub scroll_to_selected: bool,
    pub sets: Vec<SetSummary>,
    pub details_overview: Option<(TrackId, Vec<WaveCol>)>,
    pub renaming: Option<(SetId, String)>,
    pub new_set_name: String,
    pub import: Option<ImportJob>,
    pub notice: Option<(String, Instant)>,
    pub output_devices: Vec<String>,
    pub start_time: Instant,
    pub focus_search: bool,
    pub show_shortcuts: bool,
    /// Seconds of audio visible in the scrolling waveform lanes (shared, so both decks
    /// use the same time scale).
    pub wave_window_secs: f64,
    /// Text filtering the sidebar's set list by name.
    pub set_filter: String,
    /// Text filtering the tracks of the open set (the library uses `query.search`).
    pub set_track_filter: String,
    /// Level meter ballistics: channel A, channel B, master left, master right.
    pub meters: [crate::widgets::Ballistic; 4],
    meters_updated: Instant,
}

impl DjApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply(&cc.egui_ctx);
        let data_dir = djk_library::default_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        let settings = Settings::load(&data_dir);
        let mut audio = AudioService::start(settings.audio_device.as_deref());
        let ctx = cc.egui_ctx.clone();

        let mut library = match Library::open_in(&data_dir) {
            Ok(l) => Some(l),
            Err(e) => {
                tracing::error!("cannot open library: {e}");
                None
            }
        };
        let pool = library.as_mut().map(|lib| {
            let repaint = ctx.clone();
            let pool = AnalysisPool::start(&data_dir, crate::analysis::worker_count(), move || {
                repaint.request_repaint()
            });
            pool.rescan(lib);
            pool
        });
        let controller = audio.handles.controller.take().map(|queue| {
            let repaint = ctx.clone();
            ControllerManager::start(&DDJ_FLX2, queue, move || repaint.request_repaint())
        });

        let mut app = Self {
            audio,
            loader: Loader::spawn(ctx),
            library,
            data_dir,
            pool,
            controller,
            settings,
            decks: Default::default(),
            view: View::Library,
            query: TrackQuery::default(),
            rows: Vec::new(),
            rows_dirty: true,
            selected: None,
            scroll_to_selected: false,
            sets: Vec::new(),
            details_overview: None,
            renaming: None,
            new_set_name: String::new(),
            import: None,
            notice: None,
            output_devices: Vec::new(),
            start_time: Instant::now(),
            focus_search: false,
            show_shortcuts: false,
            wave_window_secs: crate::waveview::DEFAULT_WINDOW_SECS,
            set_filter: String::new(),
            set_track_filter: String::new(),
            meters: Default::default(),
            meters_updated: Instant::now(),
        };
        app.refresh_devices();
        app.reload_sets();
        app
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), Instant::now()));
    }

    pub fn deck_message(&mut self, deck: DeckId, msg: impl Into<String>) {
        self.decks[deck.index()].message = Some((msg.into(), Instant::now()));
    }

    pub fn refresh_devices(&mut self) {
        self.output_devices = djk_audio::device::list_output_devices()
            .map(|d| d.into_iter().map(|d| d.name).collect())
            .unwrap_or_default();
    }

    pub fn select_device(&mut self, name: Option<String>) {
        self.audio.switch_device(name.as_deref());
        self.settings.audio_device = name;
        self.settings.save();
    }

    pub fn selected_row(&self) -> Option<&TrackRow> {
        self.selected.and_then(|i| self.rows.get(i))
    }

    // ---- Library and sets ----

    pub fn reload_rows(&mut self) {
        self.rows_dirty = false;
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        let selected_id = self.selected.and_then(|i| self.rows.get(i)).map(|r| r.id);
        let rows = match self.view {
            View::Library => lib.tracks(&self.query),
            View::Set(set) => lib.set_tracks(set).map(|mut rows| {
                let needle = self.set_track_filter.trim().to_lowercase();
                rows.retain(|r| djk_library::tracks_match(r, &needle));
                rows
            }),
        };
        match rows {
            Ok(mut rows) => {
                // Cheap missing-file check of the rows being shown.
                let ids: Vec<TrackId> = rows.iter().map(|r| r.id).collect();
                let changed = lib.refresh_missing(&ids).unwrap_or_default();
                for r in rows.iter_mut().filter(|r| changed.contains(&r.id)) {
                    r.missing = !r.missing;
                }
                self.rows = rows;
            }
            Err(e) => self.notify(format!("Library error: {e}")),
        }
        // Keep the selected track if it is still listed; nothing is selected by default,
        // so the details panel only appears once a track is picked.
        self.selected = selected_id.and_then(|id| self.rows.iter().position(|r| r.id == id));
    }

    pub fn reload_sets(&mut self) {
        if let Some(lib) = &self.library {
            self.sets = lib.sets().unwrap_or_default();
            if let View::Set(id) = self.view
                && !self.sets.iter().any(|s| s.id == id)
            {
                self.view = View::Library;
                self.rows_dirty = true;
            }
        }
    }

    pub fn set_view(&mut self, view: View) {
        if self.view != view {
            self.view = view;
            self.selected = None;
            self.rows_dirty = true;
        }
    }

    pub fn set_sort(&mut self, key: SortKey) {
        if self.query.sort == key {
            self.query.descending = !self.query.descending;
        } else {
            self.query.sort = key;
            self.query.descending = false;
        }
        self.rows_dirty = true;
    }

    /// Run a library operation and report errors as a notice.
    pub fn with_library(&mut self, f: impl FnOnce(&mut Library) -> djk_library::Result<()>) {
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        if let Err(e) = f(lib) {
            self.notify(format!("Library error: {e}"));
        }
        self.reload_sets();
        self.rows_dirty = true;
    }

    /// Import files and/or folders on a background thread. With `into_set`, the imported
    /// tracks (new or already in the library) are also appended to that set.
    pub fn start_import(
        &mut self,
        paths: Vec<PathBuf>,
        into_set: Option<SetId>,
        ctx: &egui::Context,
    ) {
        if self.import.is_some() {
            self.notify("An import is already running — try again when it finishes");
            return;
        }
        let progress = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (tx, rx) = std::sync::mpsc::channel();
        let dir = self.data_dir.clone();
        let p = progress.clone();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("djk-import".into())
            .spawn(move || {
                let result = Library::open_in(&dir)
                    .and_then(|mut lib| {
                        let summary = lib.import_paths(&paths, |n| {
                            p.store(n, std::sync::atomic::Ordering::Relaxed);
                            if n % 25 == 0 {
                                ctx.request_repaint();
                            }
                        })?;
                        let mut added_to_set = 0;
                        if let Some(set) = into_set {
                            for &id in &summary.tracks {
                                added_to_set += usize::from(lib.add_to_set(set, id)?);
                            }
                        }
                        Ok((summary, into_set.map(|_| added_to_set)))
                    })
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
                ctx.request_repaint();
            })
            .expect("spawn import thread");
        self.import = Some(ImportJob {
            progress,
            result: rx,
        });
    }

    /// Files or folders dropped onto the window.
    pub fn handle_dropped(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        if paths.is_empty() {
            return;
        }
        let into_set = match self.view {
            View::Set(id) => Some(id),
            View::Library => None,
        };
        self.start_import(paths, into_set, ctx);
    }

    fn poll_import(&mut self) {
        let Some(job) = &self.import else { return };
        let Ok(result) = job.result.try_recv() else {
            return;
        };
        self.import = None;
        match result {
            Ok((s, added_to_set)) => {
                let mut msg = format!("Import finished: {} added", s.added);
                if s.existing > 0 {
                    msg += &format!(", {} already in library", s.existing);
                }
                if s.skipped > 0 {
                    msg += &format!(", {} skipped (unreadable)", s.skipped);
                }
                if s.added + s.existing == 0 && s.ignored > 0 {
                    msg = format!("No supported audio files ({} ignored)", s.ignored);
                }
                if let Some(n) = added_to_set {
                    msg += &format!(" · {n} added to the set");
                    self.reload_sets();
                }
                self.notify(msg);
                if let (Some(pool), Some(lib)) = (&self.pool, self.library.as_mut()) {
                    pool.rescan(lib);
                }
            }
            Err(e) => self.notify(format!("Import failed: {e}")),
        }
        self.rows_dirty = true;
    }

    fn poll_analysis(&mut self) {
        let Some(pool) = &self.pool else { return };
        let mut changed = Vec::new();
        let mut removed = 0;
        while let Some(ev) = pool.try_event() {
            match ev {
                AnalysisEvent::Analysed(id) => changed.push(id),
                AnalysisEvent::Removed(id, _) => {
                    changed.push(id);
                    removed += 1;
                }
            }
        }
        if changed.is_empty() {
            return;
        }
        if removed > 0 {
            self.notify(format!(
                "{removed} unreadable file(s) skipped and removed from the library"
            ));
        }
        self.rows_dirty = true;
        self.reload_sets();
        // Refresh deck info for analysed tracks (BPM/key appear when ready).
        if let Some(lib) = &self.library {
            for d in &mut self.decks {
                if let Some(t) = &d.track
                    && changed.contains(&t.id)
                    && let Ok(row) = lib.track(t.id)
                {
                    d.track = Some(row);
                }
            }
        }
        if self
            .details_overview
            .as_ref()
            .is_some_and(|(id, _)| changed.contains(id))
        {
            self.details_overview = None;
        }
    }

    // ---- Loading ----

    /// Load a library track onto a deck (refused if the deck is playing or the file is
    /// missing).
    pub fn load_track(&mut self, deck: DeckId, id: TrackId) {
        if self.audio.state.deck(deck).playing {
            self.deck_message(deck, "Deck is playing — pause it before loading");
            return;
        }
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        let path = match lib.loadable_path(id) {
            Ok(p) => p,
            Err(djk_library::LibraryError::Missing(_)) => {
                self.rows_dirty = true;
                self.deck_message(deck, "File is missing — cannot load");
                return;
            }
            Err(e) => {
                self.deck_message(deck, format!("Cannot load: {e}"));
                return;
            }
        };
        let track = lib.track(id).ok();
        let cache = lib.waveform_path(id);
        let ui = &mut self.decks[deck.index()];
        ui.loading = true;
        ui.message = None;
        ui.token = id.0 as u64;
        ui.track = track;
        self.loader.request(LoadJob {
            deck,
            path,
            token: id.0 as u64,
            waveform_cache: cache,
        });
    }

    pub fn load_selected(&mut self, deck: DeckId) {
        if let Some(id) = self.selected_row().map(|r| r.id) {
            self.load_track(deck, id);
        }
    }

    fn receive_loads(&mut self) {
        while let Some(done) = self.loader.try_recv() {
            let deck = done.deck;
            if self.decks[deck.index()].token != done.token {
                continue; // superseded by a later load request
            }
            self.decks[deck.index()].loading = false;
            match done.result {
                Ok((decoded, waveform, peaks)) => {
                    if self.audio.state.deck(deck).playing {
                        self.deck_message(deck, "Deck is playing — pause it before loading");
                        continue;
                    }
                    let ui = &mut self.decks[deck.index()];
                    ui.overview = crate::waveview::overview_cols(&peaks, &waveform, OVERVIEW_BINS);
                    ui.waveform = Some(Arc::new(waveform));
                    ui.peaks = Some(Arc::new(peaks));
                    if !self.audio.load(LoadRequest {
                        deck,
                        buffer: decoded.buffer,
                        token: done.token,
                    }) {
                        self.deck_message(deck, "Engine busy — try again");
                    }
                }
                Err(e) => self.deck_message(deck, format!("Could not load: {e}")),
            }
        }
    }

    // ---- Controller ----

    fn poll_controller(&mut self) {
        let mut actions = Vec::new();
        let mut events = Vec::new();
        if let Some(c) = self.controller.as_mut() {
            events = c.poll_events();
            while let Some(a) = c.try_action() {
                actions.push(a);
            }
        }
        for ev in events {
            match ev {
                djk_controller::ControllerEvent::Connected(name) => {
                    self.notify(format!("Controller connected: {name}"));
                }
                djk_controller::ControllerEvent::Disconnected => {
                    self.notify("Controller disconnected");
                }
            }
        }
        for a in actions {
            match a {
                Action::Browse(delta) => self.move_selection(delta),
                Action::LoadSelected(deck) => self.load_selected(deck),
                Action::Engine(_) => {}
            }
        }
        let time = self.start_time.elapsed().as_secs_f64();
        let state = self.audio.state;
        if let Some(c) = self.controller.as_mut() {
            c.update_leds(&state, time);
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.rows.is_empty() {
            self.selected = None;
            return;
        }
        let last = self.rows.len() as i64 - 1;
        // With nothing selected, Down starts at the top and Up at the bottom.
        let next = match self.selected {
            Some(cur) => (cur as i64 + i64::from(delta)).clamp(0, last),
            None if delta < 0 => last,
            None => 0,
        } as usize;
        self.selected = Some(next);
        self.scroll_to_selected = true;
    }

    /// Tell soft takeover that the UI changed a fader.
    pub fn ui_fader(&self, target: djk_controller::FaderTarget, value: f32) {
        if let Some(c) = &self.controller {
            c.values.set_from_ui(target, value);
        }
    }

    fn update_details(&mut self) {
        let Some(id) = self.selected_row().map(|r| r.id) else {
            return;
        };
        if self.details_overview.as_ref().is_none_or(|(d, _)| *d != id)
            && let Some(lib) = &self.library
        {
            let bins = Waveform::load(&lib.waveform_path(id))
                .map(|w| crate::waveview::cols_from_bins(&w.overview(400)))
                .unwrap_or_default();
            self.details_overview = Some((id, bins));
        }
    }
}

impl eframe::App for DjApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.audio.poll();
        self.receive_loads();
        self.poll_import();
        self.poll_analysis();
        self.poll_controller();
        if self.rows_dirty {
            self.reload_rows();
        }
        self.update_details();

        let now = Instant::now();
        let dt = now
            .duration_since(self.meters_updated)
            .as_secs_f32()
            .min(0.25);
        self.meters_updated = now;
        let s = &self.audio.state;
        let peaks = [
            s.channel_peak[0],
            s.channel_peak[1],
            s.master_peak[0],
            s.master_peak[1],
        ];
        for (m, p) in self.meters.iter_mut().zip(peaks) {
            m.update(p, dt);
        }

        let playing = self.audio.state.decks.iter().any(|d| d.playing)
            || self.meters.iter().any(|m| m.active());
        let busy = self.import.is_some() || self.pool.as_ref().is_some_and(|p| p.is_busy());
        if playing {
            ctx.request_repaint();
        } else if busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        } else {
            // Keeps LEDs blinking and the device status fresh.
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let (dropped, hovering) = ctx.input(|i| {
            let dropped: Vec<PathBuf> = i
                .raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect();
            (dropped, !i.raw.hovered_files.is_empty())
        });
        self.handle_dropped(dropped, &ctx);
        crate::shortcuts::handle(self, &ctx);
        crate::ui::layout(self, ui);
        if hovering {
            crate::ui::drop_overlay(self, &ctx);
        }
    }
}
