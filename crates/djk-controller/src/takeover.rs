//! Soft takeover for absolute controls (faders).
//!
//! A hardware fader is ignored while its physical position differs from the app's value
//! (for example after the value was changed on screen) until the hardware passes through
//! the app's value. On-screen changes are signalled through [`AppValues::set_from_ui`],
//! which bumps a generation counter the MIDI thread watches.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use djk_core::{DECK_COUNT, DeckId, EqBand, PadMode};

/// Within this distance the hardware counts as matching the app value.
pub const PICKUP_THRESHOLD: f32 = 0.02;

/// An absolute control subject to soft takeover.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FaderTarget {
    Channel(DeckId),
    Crossfader,
    Pitch(DeckId),
    /// An EQ knob (absolute, so it needs pick-up too).
    Eq(DeckId, EqBand),
    /// A filter (CFX) knob, 0 full low-pass, 0.5 off, 1 full high-pass.
    Filter(DeckId),
}

impl FaderTarget {
    pub const COUNT: usize = 13;

    pub const fn index(self) -> usize {
        match self {
            FaderTarget::Channel(d) => d.index(),
            FaderTarget::Crossfader => 2,
            FaderTarget::Pitch(d) => 3 + d.index(),
            FaderTarget::Eq(d, b) => 5 + d.index() * 3 + b.index(),
            FaderTarget::Filter(d) => 11 + d.index(),
        }
    }

    /// Value range used to normalise distances (pitch is ±8%, the others 0–1).
    fn span(self) -> f32 {
        match self {
            FaderTarget::Pitch(_) => 2.0 * djk_core::PITCH_RANGE,
            _ => 1.0,
        }
    }
}

/// App-side fader values shared with the MIDI thread.
pub struct AppValues {
    values: [AtomicU32; FaderTarget::COUNT],
    generations: [AtomicU32; FaderTarget::COUNT],
    /// Each deck's pad mode (see [`PadMode`]), shared by the screen and the controller.
    pad_modes: [AtomicU8; DECK_COUNT],
    /// Whether each deck is choosing a pad mode (after SHIFT + BEAT SYNC).
    selecting: [AtomicBool; DECK_COUNT],
}

impl Default for AppValues {
    fn default() -> Self {
        let v = |x: f32| AtomicU32::new(x.to_bits());
        let eq = djk_core::EQ_NEUTRAL;
        let filter = djk_core::FILTER_NEUTRAL;
        Self {
            // Engine defaults: faders up, crossfader centred, pitch 0, EQ and filter knobs
            // centred.
            values: [
                v(1.0),
                v(1.0),
                v(0.5),
                v(0.0),
                v(0.0),
                v(eq),
                v(eq),
                v(eq),
                v(eq),
                v(eq),
                v(eq),
                v(filter),
                v(filter),
            ],
            generations: Default::default(),
            pad_modes: Default::default(),
            selecting: Default::default(),
        }
    }
}

impl AppValues {
    pub fn get(&self, t: FaderTarget) -> f32 {
        f32::from_bits(self.values[t.index()].load(Ordering::Relaxed))
    }

    fn generation(&self, t: FaderTarget) -> u32 {
        self.generations[t.index()].load(Ordering::Acquire)
    }

    /// Record a value without signalling a takeover (used by tests and initial sync).
    pub fn set(&self, t: FaderTarget, value: f32) {
        self.values[t.index()].store(value.to_bits(), Ordering::Relaxed);
    }

    /// The UI changed this value: the hardware fader must pick it up again.
    pub fn set_from_ui(&self, t: FaderTarget, value: f32) {
        self.set(t, value);
        self.generations[t.index()].fetch_add(1, Ordering::Release);
    }

    pub fn pad_mode(&self, deck: DeckId) -> PadMode {
        match self.pad_modes[deck.index()].load(Ordering::Relaxed) {
            1 => PadMode::BeatLoop,
            _ => PadMode::HotCue,
        }
    }

    /// Set a deck's pad mode (from the screen or the controller).
    pub fn set_pad_mode(&self, deck: DeckId, mode: PadMode) {
        let v = match mode {
            PadMode::HotCue => 0,
            PadMode::BeatLoop => 1,
        };
        self.pad_modes[deck.index()].store(v, Ordering::Relaxed);
    }

    /// Whether a deck is choosing its pad mode (after SHIFT + BEAT SYNC).
    pub fn selecting(&self, deck: DeckId) -> bool {
        self.selecting[deck.index()].load(Ordering::Relaxed)
    }

    pub fn set_selecting(&self, deck: DeckId, on: bool) {
        self.selecting[deck.index()].store(on, Ordering::Relaxed);
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct FaderState {
    engaged: bool,
    last_hw: Option<f32>,
    seen_generation: u32,
}

/// Soft-takeover state for one controller connection.
pub struct Takeover {
    values: Arc<AppValues>,
    state: [FaderState; FaderTarget::COUNT],
}

impl Takeover {
    /// Faders start engaged: until something is changed on screen, the physical position
    /// is the truth, so the first message after connecting (or the controller's initial
    /// state dump) moves the app to match the hardware.
    pub fn new(values: Arc<AppValues>) -> Self {
        let mut state = [FaderState::default(); FaderTarget::COUNT];
        for (i, s) in state.iter_mut().enumerate() {
            s.seen_generation = values.generations[i].load(Ordering::Acquire);
            s.engaged = true;
        }
        Self { values, state }
    }

    /// The shared app values (also holding the pad modes).
    pub fn values(&self) -> &Arc<AppValues> {
        &self.values
    }

    /// Process a hardware value. Returns the value to apply, or `None` while waiting for
    /// the hardware to reach the app value.
    pub fn apply(&mut self, target: FaderTarget, hw: f32) -> Option<f32> {
        let generation = self.values.generation(target);
        let app = self.values.get(target);
        let span = target.span();
        let s = &mut self.state[target.index()];
        if generation != s.seen_generation {
            s.seen_generation = generation;
            s.engaged = false;
        }
        if !s.engaged {
            let close = (hw - app).abs() <= PICKUP_THRESHOLD * span;
            let crossed = s
                .last_hw
                .is_some_and(|prev| (prev - app) * (hw - app) <= 0.0);
            s.engaged = close || crossed;
        }
        s.last_hw = Some(hw);
        if s.engaged {
            // Keep the shared value current so later pickups compare against it.
            self.values.set(target, hw);
            Some(hw)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CH_A: FaderTarget = FaderTarget::Channel(DeckId::A);

    fn setup() -> (Takeover, Arc<AppValues>) {
        let values = Arc::new(AppValues::default());
        (Takeover::new(values.clone()), values)
    }

    #[test]
    fn follows_when_matching() {
        let (mut t, _) = setup();
        assert_eq!(t.apply(CH_A, 1.0), Some(1.0));
        assert_eq!(t.apply(CH_A, 0.7), Some(0.7));
    }

    #[test]
    fn value_changed_on_screen_waits_for_pickup() {
        // Spec: app lowered to 20% on screen while hardware sits at 100%; moving the
        // hardware from 100% to 90% leaves the level at 20% until it reaches 20%.
        let (mut t, values) = setup();
        assert_eq!(t.apply(CH_A, 1.0), Some(1.0));
        values.set_from_ui(CH_A, 0.2);
        for hw in [0.95, 0.9, 0.6, 0.3, 0.25] {
            assert_eq!(t.apply(CH_A, hw), None, "hw {hw}");
            assert_eq!(values.get(CH_A), 0.2);
        }
        assert_eq!(t.apply(CH_A, 0.19), Some(0.19), "crossed 20%");
        assert_eq!(t.apply(CH_A, 0.5), Some(0.5), "follows after pickup");
    }

    #[test]
    fn first_message_after_connect_is_applied() {
        // A channel fader sitting at 0 on the hardware pulls the app's (default 100%)
        // fader down as soon as it reports in, without waiting for a pickup.
        let (mut t, values) = setup();
        assert_eq!(t.apply(CH_A, 0.0), Some(0.0));
        assert_eq!(values.get(CH_A), 0.0);
        assert_eq!(t.apply(CH_A, 0.1), Some(0.1));
    }

    #[test]
    fn screen_change_before_first_message_still_needs_pickup() {
        let (mut t, values) = setup(); // app crossfader 0.5
        values.set_from_ui(FaderTarget::Crossfader, 0.5);
        assert_eq!(t.apply(FaderTarget::Crossfader, 0.0), None);
        assert_eq!(t.apply(FaderTarget::Crossfader, 0.3), None);
        assert_eq!(t.apply(FaderTarget::Crossfader, 0.6), Some(0.6));
    }

    #[test]
    fn pitch_pickup_uses_its_own_scale() {
        let (mut t, values) = setup();
        let p = FaderTarget::Pitch(DeckId::B);
        values.set_from_ui(p, 0.04);
        assert_eq!(t.apply(p, -0.08), None);
        assert_eq!(
            t.apply(p, 0.0399),
            Some(0.0399),
            "within 2% of the ±8% span"
        );
    }
}
