//! Debug-only layout dump: with `DJKITTY_DUMP_LAYOUT=1`, log the rect of every panel and
//! custom control once the layout has settled, so a restyle can be checked for moving
//! anything (compare the log before and after). It also prints frame times every 120
//! frames, to check the UI stays smooth. Compiled out of release builds.

#[cfg(debug_assertions)]
mod imp {
    use std::cell::RefCell;
    use std::sync::OnceLock;

    /// Frames seen, whether the dump is done, and the rects recorded on the dump frame.
    type DumpState = (u32, bool, Vec<(String, egui::Rect)>);

    /// Frames to wait for the layout to settle (fonts, first sizes) before dumping.
    const SETTLE_FRAMES: u32 = 30;

    fn enabled() -> bool {
        static ON: OnceLock<bool> = OnceLock::new();
        *ON.get_or_init(|| std::env::var("DJKITTY_DUMP_LAYOUT").is_ok_and(|v| v == "1"))
    }

    thread_local! {
        static STATE: RefCell<DumpState> =
            const { RefCell::new((0, false, Vec::new())) };
    }

    pub fn record(kind: &str, rect: egui::Rect) {
        if !enabled() {
            return;
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if !s.1 && s.0 == SETTLE_FRAMES {
                s.2.push((kind.to_owned(), rect));
            }
        });
    }

    thread_local! {
        /// Frame times since the last report, for the frame-rate check.
        static FRAMES: RefCell<(Option<std::time::Instant>, Vec<f32>)> =
            const { RefCell::new((None, Vec::new())) };
    }

    /// Every 120 frames, print the average and worst frame time (ms), so smoothness can be
    /// checked while the decks play.
    fn time_frame() {
        FRAMES.with(|f| {
            let mut f = f.borrow_mut();
            let now = std::time::Instant::now();
            if let Some(prev) = f.0 {
                let ms = now.duration_since(prev).as_secs_f32() * 1000.0;
                f.1.push(ms);
            }
            f.0 = Some(now);
            if f.1.len() >= 120 {
                let avg = f.1.iter().sum::<f32>() / f.1.len() as f32;
                let max = f.1.iter().copied().fold(0.0, f32::max);
                println!("frames: avg {avg:.1} ms, worst {max:.1} ms");
                f.1.clear();
            }
        });
    }

    /// Call once at the end of every frame.
    pub fn end_frame() {
        if !enabled() {
            return;
        }
        time_frame();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.1 {
                return;
            }
            if s.0 == SETTLE_FRAMES {
                let mut rows: Vec<String> =
                    s.2.iter()
                        .map(|(k, r)| {
                            format!(
                                "{k:<16} x={:>7.1} y={:>7.1} w={:>7.1} h={:>7.1}",
                                r.min.x,
                                r.min.y,
                                r.width(),
                                r.height()
                            )
                        })
                        .collect();
                rows.sort();
                println!("--- layout ({} items) ---", rows.len());
                for r in rows {
                    println!("{r}");
                }
                println!("--- end layout ---");
                s.1 = true;
            }
            s.0 += 1;
        });
    }
}

#[cfg(debug_assertions)]
pub use imp::{end_frame, record};

#[cfg(not(debug_assertions))]
pub fn record(_kind: &str, _rect: egui::Rect) {}

#[cfg(not(debug_assertions))]
pub fn end_frame() {}
