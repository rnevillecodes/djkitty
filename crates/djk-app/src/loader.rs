//! Background track loader: decodes files and prepares waveforms off the UI thread.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use djk_analysis::{Decoded, Waveform};
use djk_core::DeckId;

pub struct LoadJob {
    pub deck: DeckId,
    pub path: PathBuf,
    pub token: u64,
    /// Waveform cache file to read, if the track was analysed.
    pub waveform_cache: Option<PathBuf>,
}

pub struct Loaded {
    pub deck: DeckId,
    pub token: u64,
    pub result: Result<(Decoded, Waveform), String>,
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
                            let waveform = job
                                .waveform_cache
                                .as_deref()
                                .and_then(|p| Waveform::load(p).ok())
                                .unwrap_or_else(|| {
                                    Waveform::from_audio(
                                        &decoded.buffer.samples,
                                        decoded.buffer.sample_rate,
                                    )
                                });
                            (decoded, waveform)
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
