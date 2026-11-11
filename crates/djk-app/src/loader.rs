//! Background track loader: decodes files and prepares waveforms off the UI thread.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use djk_analysis::waveform::BINS_PER_SEC;
use djk_analysis::{Decoded, Waveform};
use djk_core::DeckId;

use crate::waveview::Peaks;

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
