//! Background track loader: decodes files and prepares waveforms off the UI thread.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use djk_analysis::waveform::BINS_PER_SEC;
use djk_analysis::{Decoded, Waveform};
use djk_core::{DeckId, TrackId};

use crate::waveview::{Peaks, WaveCol};

pub struct LoadJob {
    pub deck: DeckId,
    pub path: PathBuf,
    pub token: u64,
    /// Waveform cache file: read when it exists at the current resolution, written
    /// otherwise.
    pub waveform_cache: PathBuf,
}

pub struct Loaded {
    pub deck: DeckId,
    pub token: u64,
    /// The decoded audio, its band data (for tinting) and its min/max summary (for the
    /// waveform shape).
    pub result: Result<(Decoded, Waveform, Peaks), String>,
}

pub struct Loader {
    jobs: Sender<LoadJob>,
    results: Receiver<Loaded>,
}

impl Loader {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (job_tx, job_rx) = channel::<LoadJob>();
        let (res_tx, res_rx) = channel::<Loaded>();
        std::thread::Builder::new()
            .name("djk-loader".into())
            .spawn(move || {
                for job in job_rx {
                    let result = djk_analysis::decode_file(&job.path)
                        .map(|decoded| {
                            let cached = Waveform::load(&job.waveform_cache)
                                .ok()
                                .filter(|w| w.bins_per_sec == BINS_PER_SEC);
                            let waveform = cached.unwrap_or_else(|| {
                                let w = Waveform::from_audio(
                                    &decoded.buffer.samples,
                                    decoded.buffer.sample_rate,
                                );
                                // Missing or older, lower-resolution caches are
                                // (re)written here.
                                let _ = w.save(&job.waveform_cache);
                                w
                            });
                            let peaks = Peaks::from_stereo(
                                &decoded.buffer.samples,
                                decoded.buffer.sample_rate,
                            );
                            (decoded, waveform, peaks)
                        })
                        .map_err(|e| e.to_string());
                    let done = Loaded {
                        deck: job.deck,
                        token: job.token,
                        result,
                    };
                    if res_tx.send(done).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("spawn loader thread");
        Self {
            jobs: job_tx,
            results: res_rx,
        }
    }

    pub fn request(&self, job: LoadJob) {
        let _ = self.jobs.send(job);
    }

    pub fn try_recv(&self) -> Option<Loaded> {
        self.results.try_recv().ok()
    }
}

/// Reads waveform overviews for the track details panel off the UI thread, so browsing
/// the library never waits on the disk. Only the newest request matters: older ones still
/// queued when the worker is free are skipped.
pub struct OverviewLoader {
    jobs: Sender<(TrackId, PathBuf)>,
    results: Receiver<(TrackId, Vec<WaveCol>)>,
}

impl OverviewLoader {
    /// Columns in a details-panel overview.
    const COLS: usize = 400;

    pub fn spawn(ctx: egui::Context) -> Self {
        let (job_tx, job_rx) = channel::<(TrackId, PathBuf)>();
        let (res_tx, res_rx) = channel();
        std::thread::Builder::new()
            .name("djk-overview".into())
            .spawn(move || {
                while let Ok(mut job) = job_rx.recv() {
                    while let Ok(newer) = job_rx.try_recv() {
                        job = newer;
                    }
                    let (id, path) = job;
                    // No cache file yet (not analysed): an empty overview.
                    let cols = Waveform::load(&path)
                        .map(|w| crate::waveview::cols_from_bins(&w.overview(Self::COLS)))
                        .unwrap_or_default();
                    if res_tx.send((id, cols)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("spawn overview thread");
        Self {
            jobs: job_tx,
            results: res_rx,
        }
    }

    /// Ask for the overview stored in the waveform cache file `path`.
    pub fn request(&self, id: TrackId, path: PathBuf) {
        let _ = self.jobs.send((id, path));
    }

    pub fn try_recv(&self) -> Option<(TrackId, Vec<WaveCol>)> {
        self.results.try_recv().ok()
    }
}
