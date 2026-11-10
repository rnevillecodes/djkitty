//! Enforces the real-time rule: `Engine::process` performs no heap allocation or free once
//! warmed up, including while applying commands, loading tracks and retiring buffers.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use djk_audio::Engine;
use djk_core::{Command, DeckId, LoadRequest, TrackBuffer};

struct CountingAlloc;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static FREES: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

fn counting() -> bool {
    COUNTING.try_with(|c| c.get()).unwrap_or(false)
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if counting() {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if counting() {
            FREES.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if counting() {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

fn tone(secs: f32) -> TrackBuffer {
    let n = (48_000.0 * secs) as usize;
    let s: Vec<f32> = (0..n)
        .flat_map(|i| {
            let v = (i as f32 * 0.03).sin();
            [v, v]
        })
        .collect();
    TrackBuffer {
        samples: Arc::from(s),
        sample_rate: 44_100,
    }
}

fn measure(f: impl FnOnce()) -> (usize, usize) {
    ALLOCS.store(0, Ordering::SeqCst);
    FREES.store(0, Ordering::SeqCst);
    COUNTING.with(|c| c.set(true));
    f();
    COUNTING.with(|c| c.set(false));
    (ALLOCS.load(Ordering::SeqCst), FREES.load(Ordering::SeqCst))
}

#[test]
fn counting_allocator_detects_allocations() {
    let (allocs, frees) = measure(|| {
        let v = std::hint::black_box(vec![1u8; 64]);
        drop(v);
    });
    assert!(allocs >= 1 && frees >= 1);
}

#[test]
fn process_never_allocates_or_frees() {
    let (mut engine, mut h) = Engine::new(48_000, 4);
    let mut out = vec![0.0f32; 4096 * 4];

    // Warm up.
    engine.process(&mut out, 4, Some(Instant::now()));

    // Queue a busy workload before measuring: loads, replacement loads, commands.
    for (deck, token) in [(DeckId::A, 1), (DeckId::B, 2)] {
        h.load(LoadRequest {
            deck,
            buffer: tone(2.0),
            token,
        })
        .unwrap();
    }
    let mut cmds = vec![
        Command::Play(DeckId::A),
        Command::Play(DeckId::B),
        Command::SetPitch {
            deck: DeckId::A,
            pitch: 0.05,
        },
        Command::Jog {
            deck: DeckId::B,
            ticks: 3.0,
        },
        Command::SetCrossfader(0.2),
        Command::ToggleHeadphoneCue(DeckId::A),
        Command::CuePress(DeckId::B),
        Command::CueRelease(DeckId::B),
    ];
    for c in cmds.drain(..) {
        h.send(c);
    }

    let (allocs, frees) = measure(|| {
        for _ in 0..20 {
            engine.process(&mut out, 4, Some(Instant::now()));
        }
    });
    assert_eq!((allocs, frees), (0, 0), "while playing");

    // Replacement load on a paused deck retires the old buffer — still no frees here.
    h.send(Command::Pause(DeckId::A));
    h.load(LoadRequest {
        deck: DeckId::A,
        buffer: tone(1.0),
        token: 3,
    })
    .unwrap();
    // Refused load onto playing deck B is retired too.
    h.load(LoadRequest {
        deck: DeckId::B,
        buffer: tone(1.0),
        token: 4,
    })
    .unwrap();
    let (allocs, frees) = measure(|| {
        for _ in 0..5 {
            engine.process(&mut out, 4, Some(Instant::now()));
        }
    });
    assert_eq!((allocs, frees), (0, 0), "while loading and retiring");

    // The buffers are freed on this (non-audio) side instead.
    assert_eq!(h.drop_retired(), 2);
}
