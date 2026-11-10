//! Background analysis pool: analyses new or changed tracks on low-priority worker
//! threads and stores results and waveforms in the library.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use djk_core::TrackId;
use djk_library::{AnalysisRecord, Library};

/// What the UI hears from the pool.
#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisEvent {
    /// A track's analysis was stored.
    Analysed(TrackId),
    /// A track could not be decoded and was removed from the library.
    Removed(TrackId, String),
}

#[derive(Default)]
struct Queue {
    jobs: VecDeque<(TrackId, PathBuf)>,
    /// Queued or being analysed.
    pending: HashSet<TrackId>,
    shutdown: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
    total: AtomicUsize,
    done: AtomicUsize,
}

pub struct AnalysisPool {
    shared: Arc<Shared>,
    events: Receiver<AnalysisEvent>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

/// Number of worker threads: CPUs − 2, at least 1.
pub fn worker_count() -> usize {
    num_cpus::get().saturating_sub(2).max(1)
}

impl AnalysisPool {
    /// Start `workers` threads analysing tracks of the library in `db_dir`. `notify` is
    /// called after each event (e.g. to request a UI repaint).
    pub fn start(db_dir: &Path, workers: usize, notify: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
            total: AtomicUsize::new(0),
            done: AtomicUsize::new(0),
        });
        let (tx, rx) = channel();
        let notify = Arc::new(notify);
        let handles = (0..workers.max(1))
            .map(|i| {
                let shared = shared.clone();
                let tx = tx.clone();
                let dir = db_dir.to_path_buf();
                let notify = notify.clone();
                std::thread::Builder::new()
                    .name(format!("djk-analysis-{i}"))
                    .spawn(move || worker(shared, dir, tx, move || notify()))
                    .expect("spawn analysis worker")
            })
            .collect();
        Self {
            shared,
            events: rx,
            workers: handles,
        }
    }

    /// Queue every track that needs (re-)analysis and is not already queued. Returns how
    /// many were added.
    pub fn rescan(&self, lib: &mut Library) -> usize {
        let needed = match lib.tracks_needing_analysis() {
            Ok(n) => n,
            Err(e) => {
                tracing::error!("analysis rescan failed: {e}");
                return 0;
            }
        };
        let mut q = self.shared.queue.lock().unwrap();
        if q.pending.is_empty() {
            // Previous batch finished: restart the progress counters.
            self.shared.total.store(0, Ordering::Relaxed);
            self.shared.done.store(0, Ordering::Relaxed);
        }
        let mut added = 0;
        for (id, path) in needed {
            if q.pending.insert(id) {
                q.jobs.push_back((id, path));
                added += 1;
            }
        }
        self.shared.total.fetch_add(added, Ordering::Relaxed);
        drop(q);
        self.shared.wake.notify_all();
        added
    }

    /// `(done, total)` for the current batch.
    pub fn progress(&self) -> (usize, usize) {
        (
            self.shared.done.load(Ordering::Relaxed),
            self.shared.total.load(Ordering::Relaxed),
        )
    }

    pub fn is_busy(&self) -> bool {
        let (done, total) = self.progress();
        done < total
    }

    pub fn try_event(&self) -> Option<AnalysisEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for AnalysisPool {
    fn drop(&mut self) {
        {
            let mut q = self.shared.queue.lock().unwrap();
            q.shutdown = true;
            q.jobs.clear();
        }
        self.shared.wake.notify_all();
        for h in self.workers.drain(..) {
            let _ = h.join();
        }
    }
}

fn worker(shared: Arc<Shared>, dir: PathBuf, tx: Sender<AnalysisEvent>, notify: impl Fn()) {
    lower_thread_priority();
    let mut lib = match Library::open_in(&dir) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("analysis worker cannot open library: {e}");
            return;
        }
    };
    loop {
        let job = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if q.shutdown {
                    return;
                }
                if let Some(job) = q.jobs.pop_front() {
                    break job;
                }
                q = shared.wake.wait(q).unwrap();
            }
        };
        let (id, path) = job;
        let event = analyse_one(&mut lib, id, &path);
        shared.queue.lock().unwrap().pending.remove(&id);
        shared.done.fetch_add(1, Ordering::Relaxed);
        if let Some(ev) = event {
            let _ = tx.send(ev);
        }
        notify();
    }
}

fn analyse_one(lib: &mut Library, id: TrackId, path: &Path) -> Option<AnalysisEvent> {
    let stamp = Library::stamp(path).ok()?;
    match djk_analysis::analyse_file(path) {
        Ok(a) => {
            if let Err(e) = a.waveform.save(&lib.waveform_path(id)) {
                tracing::warn!("cannot write waveform for {}: {e}", path.display());
            }
            let rec = AnalysisRecord {
                duration_secs: a.stats.duration_secs,
                format: a.stats.format,
                sample_rate: a.stats.sample_rate,
                channels: a.stats.channels,
                bitrate_kbps: a.stats.bitrate_kbps,
                lossless: a.stats.lossless,
                bpm: a.bpm,
                key: a.key,
            };
            match lib.save_analysis(id, &rec, stamp) {
                Ok(()) => Some(AnalysisEvent::Analysed(id)),
                Err(e) => {
                    tracing::error!("cannot save analysis for {}: {e}", path.display());
                    None
                }
            }
        }
        Err(djk_analysis::DecodeError::Io(e)) => {
            tracing::warn!("cannot read {}: {e}", path.display());
            None
        }
        Err(e) => {
            tracing::warn!("removing undecodable {}: {e}", path.display());
            let _ = lib.remove_track(id);
            Some(AnalysisEvent::Removed(id, e.to_string()))
        }
    }
}

#[cfg(windows)]
fn lower_thread_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
    };
    // SAFETY: GetCurrentThread returns a pseudo-handle valid for the calling thread.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

#[cfg(not(windows))]
fn lower_thread_priority() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../djk-analysis/tests/fixtures")
            .join(name)
    }

    fn wait_idle(pool: &AnalysisPool) {
        let start = Instant::now();
        while pool.is_busy() {
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "analysis timed out"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn analyses_once_and_again_after_file_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let music = tmp.path().join("music");
        let data = tmp.path().join("data");
        std::fs::create_dir_all(&music).unwrap();
        for (name, to) in [
            ("tone.mp3", "a.mp3"),
            ("tone.flac", "b.flac"),
            ("tone.wav", "c.wav"),
        ] {
            std::fs::copy(fixture(name), music.join(to)).unwrap();
        }
        let mut lib = Library::open_in(&data).unwrap();
        lib.import_folder(&music, |_| {}).unwrap();

        let pool = AnalysisPool::start(&data, 2, || {});
        assert_eq!(pool.rescan(&mut lib), 3);
        wait_idle(&pool);
        assert_eq!(pool.progress(), (3, 3));
        let mut analysed = 0;
        while let Some(ev) = pool.try_event() {
            assert!(matches!(ev, AnalysisEvent::Analysed(_)));
            analysed += 1;
        }
        assert_eq!(analysed, 3);
        for t in lib.tracks(&Default::default()).unwrap() {
            assert!(t.analysed);
            assert!(lib.waveform_path(t.id).is_file());
            assert!(djk_analysis::Waveform::load(&lib.waveform_path(t.id)).is_ok());
        }
        drop(pool);

        // "Restart": a fresh pool finds nothing to do.
        let pool = AnalysisPool::start(&data, 2, || {});
        assert_eq!(pool.rescan(&mut lib), 0);

        // Replace one file with different content at the same path.
        std::fs::copy(fixture("tone.m4a"), music.join("c.wav")).unwrap();
        assert_eq!(pool.rescan(&mut lib), 1);
        wait_idle(&pool);
        assert_eq!(pool.progress(), (1, 1));
    }

    #[test]
    fn worker_count_leaves_cores_free() {
        assert!(worker_count() >= 1);
        assert!(worker_count() <= num_cpus::get().max(1));
    }
}
