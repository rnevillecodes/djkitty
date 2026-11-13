//! The main application: state, background services and per-frame wiring.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use djk_analysis::Waveform;
use djk_analysis::waveform::OVERVIEW_BINS;
use djk_controller::{Action, ControllerManager, DDJ_FLX2};
use djk_core::{Command, DeckId, HOT_CUE_COUNT, HotCues, LoadRequest, PadMode, TrackId};
use djk_library::{ImportSummary, Library, SetId, SetSummary, SortKey, TrackQuery, TrackRow};

use crate::analysis::{AnalysisEvent, AnalysisPool};
use crate::audio::AudioService;
use crate::loader::{LoadJob, Loader, OverviewLoader};
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
    /// Each band's typical level in this track, for tinting (see `waveview::band_norms`).
    pub band_norm: [f32; 3],
    /// The track's beatgrid (user-edited or detected), if it has one.
    pub grid: Option<djk_library::StoredGrid>,
    /// Whether the grid editing tools are open in this deck's waveform lane.
    pub grid_editing: bool,
    pub overview: Vec<WaveCol>,
    /// A track on its way to this deck. Everything above keeps describing the track the
    /// engine holds until the engine reports it has taken this one.
    pub pending: Option<PendingLoad>,
    pub message: Option<(String, Instant)>,
    pub cue_held: bool,
    /// The engine's hot cues (frames) as last seen for `token`, to spot changes to save.
    pub seen_hot_cues: Option<HotCues>,
    /// The beat length last sent to the engine for `token`.
    pub sent_beat_frames: Option<Option<f64>>,
    /// The engine's refused-loop count as last seen.
    pub seen_loop_refusals: Option<u32>,
    /// What the on-screen pads (and the controller's pads) do for this deck.
    pub pad_mode: PadMode,
    /// The on-screen hot cue pad held down with the mouse, if any.
    pub pad_held: Option<u8>,
}

impl DeckUi {
    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }
}

/// A track being decoded for a deck, then sent to the engine. It only becomes the deck's
/// track (see [`DeckUi`]) once the engine holds it, so a refused load leaves the deck
/// describing what it is really playing.
pub struct PendingLoad {
    pub token: u64,
    pub track: Option<TrackRow>,
    pub grid: Option<djk_library::StoredGrid>,
    /// The track's saved hot cues (track seconds), sent with its load request.
    pub hot_cues: HotCues,
    /// Set once decoded and handed to the engine.
    pub sent: Option<SentLoad>,
}

/// What a decoded track brings to its deck once the engine takes it.
pub struct SentLoad {
    waveform: Arc<Waveform>,
    peaks: Arc<Peaks>,
    overview: Vec<WaveCol>,
    band_norm: [f32; 3],
    /// The deck's refused-load count when this load was sent.
    load_refusals: u32,
}

/// What the "Remove from library?" dialog is asking about.
pub struct RemovePrompt {
    pub id: TrackId,
    pub title: String,
    /// Number of sets the track is in (it leaves them too).
    pub sets: usize,
}

/// What the "Clear hot cues?" dialog is asking about.
pub struct ClearCuesPrompt {
    pub id: TrackId,
    pub title: String,
    /// Number of hot cues the track has (all of them go).
    pub count: usize,
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
    /// Re-check which listed files exist on the next reload. Set when the list itself
    /// changes (another view, an import), not for every reload: it reads the disk once
    /// per track.
    check_missing: bool,
    pub selected: Option<usize>,
    pub scroll_to_selected: bool,
    pub sets: Vec<SetSummary>,
    pub details_overview: Option<(TrackId, Vec<WaveCol>)>,
    /// The track whose overview was last asked of `overview_loader`.
    details_requested: Option<TrackId>,
    overview_loader: OverviewLoader,
    /// The stored analysis of the track in the details panel, read once per track (see
    /// [`DjApp::details_analysis`]).
    details_analysis: Option<(TrackId, Option<djk_library::AnalysisRecord>)>,
    pub renaming: Option<(SetId, String)>,
    pub new_set_name: String,
    /// Whether the "New set" dialog is open.
    pub new_set_dialog: bool,
    /// A pending "Remove from library?" confirmation.
    pub remove_prompt: Option<RemovePrompt>,
    pub clear_cues_prompt: Option<ClearCuesPrompt>,
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
    /// How the open set is sorted: `None` is the set's own order (the "#" column).
    pub set_sort: Option<SortKey>,
    pub set_sort_desc: bool,
    /// Set position (from 1) of each row in the set view.
    pub row_numbers: Vec<usize>,
    /// Track list column widths (title, artist, BPM, key, LUFS, time).
    pub track_columns: crate::columns::FitColumns,
    /// Level meter ballistics: channel A, channel B, master left, master right.
    pub meters: [crate::widgets::Ballistic; 4],
    meters_updated: Instant,
}

impl DjApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        crate::theme::apply(&cc.egui_ctx);
        let data_dir = djk_library::default_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        let settings = Settings::load(&data_dir);
        let settings_columns = settings.track_columns.clone();
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
            loader: Loader::spawn(ctx.clone()),
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
            check_missing: true,
            selected: None,
            scroll_to_selected: false,
            sets: Vec::new(),
            details_overview: None,
            details_requested: None,
            overview_loader: OverviewLoader::spawn(ctx),
            details_analysis: None,
            renaming: None,
            new_set_name: String::new(),
            new_set_dialog: false,
            remove_prompt: None,
            clear_cues_prompt: None,
            import: None,
            notice: None,
            output_devices: Vec::new(),
            start_time: Instant::now(),
            focus_search: false,
            show_shortcuts: false,
            wave_window_secs: crate::waveview::DEFAULT_WINDOW_SECS,
            set_filter: String::new(),
            set_track_filter: String::new(),
            set_sort: None,
            set_sort_desc: false,
            row_numbers: Vec::new(),
            track_columns: crate::ui::track_columns(settings_columns.as_deref()),
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
        let mut numbers = Vec::new();
        let rows = match self.view {
            View::Library => lib.tracks(&self.query),
            View::Set(set) => lib.set_tracks(set).map(|rows| {
                // Number the tracks in set order before filtering and sorting.
                let needle = self.set_track_filter.trim().to_lowercase();
                let mut numbered: Vec<(usize, TrackRow)> = rows
                    .into_iter()
                    .enumerate()
                    .map(|(i, r)| (i + 1, r))
                    .filter(|(_, r)| djk_library::tracks_match(r, &needle))
                    .collect();
                match self.set_sort {
                    Some(key) => {
                        let mut rows: Vec<TrackRow> =
                            numbered.iter().map(|(_, r)| r.clone()).collect();
                        djk_library::sort_tracks(&mut rows, key, self.set_sort_desc);
                        let position: std::collections::HashMap<TrackId, usize> =
                            numbered.iter().map(|(n, r)| (r.id, *n)).collect();
                        numbers = rows.iter().map(|r| position[&r.id]).collect();
                        rows
                    }
                    None => {
                        if self.set_sort_desc {
                            numbered.reverse();
                        }
                        numbers = numbered.iter().map(|(n, _)| *n).collect();
                        numbered.into_iter().map(|(_, r)| r).collect()
                    }
                }
            }),
        };
        self.row_numbers = numbers;
        match rows {
            Ok(mut rows) => {
                if std::mem::take(&mut self.check_missing) {
                    let ids: Vec<TrackId> = rows.iter().map(|r| r.id).collect();
                    let changed: std::collections::HashSet<TrackId> = lib
                        .refresh_missing(&ids)
                        .unwrap_or_default()
                        .into_iter()
                        .collect();
                    for r in rows.iter_mut().filter(|r| changed.contains(&r.id)) {
                        r.missing = !r.missing;
                    }
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
            self.check_missing = true;
        }
    }

    /// Sort the list by a column; clicking the current column again reverses it. In a
    /// set, `None` is the set's own order (the "#" column).
    pub fn set_sort(&mut self, key: Option<SortKey>) {
        match (self.view, key) {
            (View::Library, None) => return,
            (View::Library, Some(key)) => {
                if self.query.sort == key {
                    self.query.descending = !self.query.descending;
                } else {
                    self.query.sort = key;
                    self.query.descending = false;
                }
            }
            (View::Set(_), key) => {
                if self.set_sort == key {
                    self.set_sort_desc = !self.set_sort_desc;
                } else {
                    self.set_sort = key;
                    self.set_sort_desc = false;
                }
            }
        }
        self.rows_dirty = true;
    }

    /// Whether the open set is shown in its own order, so rows can be dragged to reorder.
    pub fn set_in_own_order(&self) -> bool {
        self.set_sort.is_none() && !self.set_sort_desc
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

    // ---- Beatgrid editing. Grids are a visual aid for beatmatching by ear and eye;
    // djkitty never syncs decks. Edits are saved in the library, never in the audio file.

    /// Replace a deck's grid with an edited one. With `save`, it is stored for the track
    /// (live drags pass `false` and save once when the drag ends).
    pub fn set_deck_grid(&mut self, deck: DeckId, grid: djk_core::Beatgrid, save: bool) {
        let d = &mut self.decks[deck.index()];
        d.grid = Some(djk_library::StoredGrid { grid, edited: true });
        let Some(id) = d.track.as_ref().map(|t| t.id) else {
            return;
        };
        if save {
            self.with_library(|lib| lib.set_beatgrid(id, grid));
            // Keep the deck's BPM readout in step with the edited tempo.
            if let Some(lib) = &self.library
                && let Ok(row) = lib.track(id)
            {
                self.decks[deck.index()].track = Some(row);
            }
        }
    }

    /// Drop a deck's grid edits and go back to the detected grid.
    pub fn reset_deck_grid(&mut self, deck: DeckId) {
        let Some(id) = self.decks[deck.index()].track.as_ref().map(|t| t.id) else {
            return;
        };
        self.with_library(|lib| lib.reset_beatgrid(id));
        if let Some(lib) = &self.library {
            let d = &mut self.decks[deck.index()];
            d.grid = lib.beatgrid(id).ok().flatten();
            if let Ok(row) = lib.track(id) {
                d.track = Some(row);
            }
        }
    }

    // ---- Track management. None of this touches audio files on disk: removing only
    // changes the library database (and drops djkitty's own cached waveform).

    /// Ask to remove a track from the library (opens a confirmation dialog).
    pub fn ask_remove_from_library(&mut self, id: TrackId) {
        let Some(lib) = &self.library else { return };
        let title = lib.track(id).map(|t| t.title).unwrap_or_default();
        let sets = lib.sets_containing(id).unwrap_or(0);
        self.remove_prompt = Some(RemovePrompt { id, title, sets });
    }

    /// Ask to clear all of a track's hot cues (opens a confirmation dialog).
    pub fn ask_clear_hot_cues(&mut self, id: TrackId) {
        let Some(lib) = &self.library else { return };
        let title = lib.track(id).map(|t| t.title).unwrap_or_default();
        let count = self.hot_cue_count(id);
        if count > 0 {
            self.clear_cues_prompt = Some(ClearCuesPrompt { id, title, count });
        }
    }

    /// How many hot cues a track has: the deck's live ones if it is loaded (they may be a
    /// frame ahead of the library), otherwise the saved ones.
    pub fn hot_cue_count(&self, id: TrackId) -> usize {
        let token = id.0 as u64;

        let live = DeckId::ALL
            .into_iter()
            .filter(|&d| self.decks[d.index()].token == token && self.audio.state.deck(d).loaded)
            .map(|d| self.audio.state.deck(d).hot_cues.iter().flatten().count())
            .max();
        live.or_else(|| {
            let lib = self.library.as_ref()?;
            lib.hot_cues(id).ok().map(|c| c.iter().flatten().count())
        })
        .unwrap_or(0)
    }

    /// Clear all of a track's hot cues. A deck holding the track (or loading it) clears
    /// them in the engine (whose slots are the source of truth while loaded, then saved as
    /// usual); the library copy is cleared too, for when it isn't loaded.
    pub fn clear_hot_cues(&mut self, id: TrackId) {
        let token = id.0 as u64;
        for deck in DeckId::ALL {
            let ui = &mut self.decks[deck.index()];
            let mut holds = ui.token == token;
            if let Some(p) = ui.pending.as_mut().filter(|p| p.token == token) {
                p.hot_cues = [None; HOT_CUE_COUNT];
                holds = true;
            }
            if holds {
                // Ordered after any queued load, and ignored unless the deck holds `token`.
                self.audio.send(Command::ClearHotCues { deck, token });
            }
        }
        self.with_library(|lib| lib.set_hot_cues(id, &[None; HOT_CUE_COUNT]));
    }

    /// Remove a track from the library (and every set). The file stays on disk.
    pub fn remove_from_library(&mut self, id: TrackId) {
        let title = self
            .library
            .as_ref()
            .and_then(|l| l.track(id).ok())
            .map(|t| t.title)
            .unwrap_or_default();
        self.with_library(|lib| lib.remove_track(id));
        self.forget_details_overview(&[id]);
        self.forget_details_analysis(&[id]);
        self.notify(format!(
            "Removed “{title}” from the library (the file is still on your disk)"
        ));
    }

    /// Remove the track at `row` from the open set. It stays in the library.
    pub fn remove_from_open_set(&mut self, row: usize) {
        let View::Set(set) = self.view else { return };
        let Some(track) = self.rows.get(row).cloned() else {
            return;
        };
        self.with_library(|lib| lib.remove_from_set(set, track.id));
        self.notify(format!("Removed “{}” from this set", track.title));
    }

    /// The Delete key: remove the selected track from the open set, or (in the library)
    /// ask to remove it from the library.
    pub fn remove_selected(&mut self) {
        let Some(row) = self.selected else { return };
        match self.view {
            View::Set(_) => self.remove_from_open_set(row),
            View::Library => {
                if let Some(id) = self.rows.get(row).map(|r| r.id) {
                    self.ask_remove_from_library(id);
                }
            }
        }
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
        self.check_missing = true;
    }

    fn poll_analysis(&mut self) {
        let Some(pool) = &self.pool else { return };
        let mut changed = Vec::new();
        let mut failed = 0;
        while let Some(ev) = pool.try_event() {
            match ev {
                AnalysisEvent::Analysed(id) => changed.push(id),
                AnalysisEvent::Failed(..) => failed += 1,
            }
        }
        if failed > 0 {
            self.notify(format!(
                "{failed} file(s) could not be analysed (they stay in the library)"
            ));
        }
        if changed.is_empty() {
            return;
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
                    d.grid = lib.beatgrid(row.id).ok().flatten();
                    d.track = Some(row);
                }
            }
        }
        self.forget_details_overview(&changed);
        self.forget_details_analysis(&changed);
    }

    /// The stored analysis of the track shown in the details panel. Read from the
    /// library the first time it is asked for a track and kept until the track is
    /// re-analysed or removed, because the panel asks again on every frame.
    pub fn details_analysis(&mut self, id: TrackId) -> Option<djk_library::AnalysisRecord> {
        if self.details_analysis.as_ref().is_none_or(|(d, _)| *d != id) {
            let analysis = self
                .library
                .as_ref()
                .and_then(|l| l.analysis(id).ok().flatten());
            self.details_analysis = Some((id, analysis));
        }
        self.details_analysis.as_ref().and_then(|(_, a)| a.clone())
    }

    fn forget_details_analysis(&mut self, ids: &[TrackId]) {
        if self
            .details_analysis
            .as_ref()
            .is_some_and(|(id, _)| ids.contains(id))
        {
            self.details_analysis = None;
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
        if self.decks[deck.index()]
            .pending
            .as_ref()
            .is_some_and(|p| p.sent.is_some())
        {
            // The engine is taking the previous load (a block or two); replacing it now
            // would leave the deck unsure which track it holds.
            self.deck_message(deck, "Still loading — try again");
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
        let grid = lib.beatgrid(id).ok().flatten();
        let hot_cues = lib.hot_cues(id).unwrap_or_default();
        let cache = lib.waveform_path(id);
        let ui = &mut self.decks[deck.index()];
        ui.message = None;
        ui.pending = Some(PendingLoad {
            token: id.0 as u64,
            track,
            grid,
            hot_cues,
            sent: None,
        });
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
            let ui = &mut self.decks[deck.index()];
            let Some(pending) = ui
                .pending
                .as_mut()
                .filter(|p| p.token == done.token && p.sent.is_none())
            else {
                continue; // superseded by a later load request
            };
            let (decoded, waveform, peaks) = match done.result {
                Ok(r) => r,
                Err(e) => {
                    ui.pending = None;
                    self.deck_message(deck, format!("Could not load: {e}"));
                    continue;
                }
            };
            let state = *self.audio.state.deck(deck);
            if state.playing {
                ui.pending = None;
                self.deck_message(deck, "Deck is playing — pause it before loading");
                continue;
            }
            let rate = f64::from(decoded.buffer.sample_rate);
            let hot_cues = pending.hot_cues.map(|c| c.map(|secs| secs * rate));
            let sent = SentLoad {
                overview: crate::waveview::overview_cols(&peaks, &waveform, OVERVIEW_BINS),
                band_norm: crate::waveview::band_norms(&waveform.bins),
                waveform: Arc::new(waveform),
                peaks: Arc::new(peaks),
                load_refusals: state.load_refusals,
            };
            if self.audio.load(LoadRequest {
                deck,
                buffer: decoded.buffer,
                token: done.token,
                hot_cues,
            }) {
                // Becomes the deck's track in `settle_loads`, once the engine has it.
                pending.sent = Some(sent);
            } else {
                ui.pending = None;
                self.deck_message(deck, "Engine busy — try again");
            }
        }
    }

    /// Finish loads the engine has dealt with: take on the track it accepted, or drop
    /// one it refused (the deck started playing before the load arrived).
    fn settle_loads(&mut self) {
        for deck in DeckId::ALL {
            let s = *self.audio.state.deck(deck);
            let ui = &mut self.decks[deck.index()];
            let Some(pending) = &ui.pending else { continue };
            let Some(sent) = &pending.sent else { continue };
            if s.load_refusals != sent.load_refusals {
                ui.pending = None;
                self.deck_message(deck, "Deck is playing — pause it before loading");
                continue;
            }
            if !s.loaded || s.track_token != pending.token {
                continue; // not processed yet
            }
            let Some(PendingLoad {
                token,
                track,
                grid,
                sent: Some(sent),
                ..
            }) = ui.pending.take()
            else {
                continue;
            };
            ui.token = token;
            ui.track = track;
            ui.grid = grid;
            ui.grid_editing = false;
            ui.waveform = Some(sent.waveform);
            ui.peaks = Some(sent.peaks);
            ui.overview = sent.overview;
            ui.band_norm = sent.band_norm;
            ui.seen_hot_cues = None;
            ui.sent_beat_frames = None;
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
        // The controller may have switched a deck's pads (SHIFT + BEAT SYNC); mirror it.
        if let Some(c) = &self.controller {
            for deck in DeckId::ALL {
                let mode = c.values.pad_mode(deck);
                let d = &mut self.decks[deck.index()];
                if d.pad_mode != mode {
                    if let Some(slot) = d.pad_held.take() {
                        self.audio.send(Command::HotCueRelease { deck, slot });
                    }
                    self.decks[deck.index()].pad_mode = mode;
                }
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

    /// Keep each deck's engine-side extras in step with the app: send the beat length
    /// when the track's tempo is known or changes, save hot cues when they change (from
    /// the pads or the screen), and explain refused loops.
    fn sync_decks(&mut self) {
        for deck in DeckId::ALL {
            let s = *self.audio.state.deck(deck);
            let ui = &mut self.decks[deck.index()];
            if let Some(seen) = ui.seen_loop_refusals
                && s.loop_refusals != seen
            {
                ui.message = Some(("Needs a BPM to loop".into(), Instant::now()));
            }
            ui.seen_loop_refusals = Some(s.loop_refusals);
            // Only act once the engine holds the track this deck's info is about. (A load
            // in flight doesn't stop this: until it lands, edits to the current track
            // still need saving.)
            if !s.loaded || s.track_token != ui.token {
                continue;
            }
            let rate = f64::from(s.track_rate.max(1));
            let beat = ui
                .grid
                .map(|g| g.grid.bpm)
                .filter(|bpm| *bpm > 0.0)
                .map(|bpm| 60.0 / bpm * rate);
            if ui.sent_beat_frames != Some(beat) {
                ui.sent_beat_frames = Some(beat);
                self.audio.send(Command::SetBeatLength {
                    deck,
                    token: ui.token,
                    frames: beat.unwrap_or(0.0),
                });
            }
            match ui.seen_hot_cues {
                None => ui.seen_hot_cues = Some(s.hot_cues),
                Some(seen) if seen != s.hot_cues => {
                    ui.seen_hot_cues = Some(s.hot_cues);
                    let id = TrackId(ui.token as i64);
                    // Straight to the library, not `with_library`: hot cues appear in
                    // neither the track list nor the sets, so there is nothing to reload.
                    let saved = self.library.as_mut().map(|lib| {
                        // Write only the slots this deck changed, so another deck holding
                        // the same track (with its own copy of the cues) can't undo edits
                        // made there.
                        let mut cues = lib.hot_cues(id)?;
                        for (slot, (now, before)) in s.hot_cues.iter().zip(seen).enumerate() {
                            if *now != before {
                                cues[slot] = now.map(|f| f / rate);
                            }
                        }
                        match lib.set_hot_cues(id, &cues) {
                            // Removed from the library while loaded: nothing to keep.
                            Err(djk_library::LibraryError::NotFound) => Ok(()),
                            r => r,
                        }
                    });
                    if let Some(Err(e)) = saved {
                        self.notify(format!("Library error: {e}"));
                    }
                }
                Some(_) => {}
            }
        }
    }

    /// Switch what a deck's pads do, on screen and on the controller.
    pub fn set_pad_mode(&mut self, deck: DeckId, mode: PadMode) {
        let d = &mut self.decks[deck.index()];
        if let Some(slot) = d.pad_held.take() {
            self.audio.send(Command::HotCueRelease { deck, slot });
        }
        self.decks[deck.index()].pad_mode = mode;
        if let Some(c) = &self.controller {
            c.values.set_pad_mode(deck, mode);
            c.values.set_selecting(deck, false);
        }
    }

    /// Tell soft takeover that the UI changed a fader.
    pub fn ui_fader(&self, target: djk_controller::FaderTarget, value: f32) {
        if let Some(c) = &self.controller {
            c.values.set_from_ui(target, value);
        }
    }

    /// Keep the details panel's overview on the selected track. The waveform file is read
    /// on a background thread, so the overview appears a moment after the selection moves.
    fn update_details(&mut self) {
        while let Some((id, cols)) = self.overview_loader.try_recv() {
            // Anything but the latest request has been overtaken.
            if self.details_requested == Some(id) {
                self.details_overview = Some((id, cols));
            }
        }
        let Some(id) = self.selected_row().map(|r| r.id) else {
            return;
        };
        if self.details_requested != Some(id)
            && let Some(lib) = &self.library
        {
            self.details_requested = Some(id);
            self.overview_loader.request(id, lib.waveform_path(id));
        }
    }

    /// Drop the details panel's overview if it is for one of `ids`, so it is read again.
    fn forget_details_overview(&mut self, ids: &[TrackId]) {
        if self.details_requested.is_some_and(|id| ids.contains(&id)) {
            self.details_requested = None;
            self.details_overview = None;
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
        self.settle_loads();
        self.sync_decks();
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
        crate::layout_dump::end_frame();
    }
}
