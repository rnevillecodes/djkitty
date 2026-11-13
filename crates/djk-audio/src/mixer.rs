//! Mixer: channel faders, crossfader, master and headphone buses, and a peak limiter.
//!
//! Runs on the audio thread: no allocation, locking or I/O.

use djk_core::{DECK_COUNT, MixerState};

/// Limiter ceiling: −0.3 dBFS.
pub const LIMITER_CEILING: f32 = 0.966_051; // 10^(-0.3/20)
/// Limiter release time constant in seconds.
pub const LIMITER_RELEASE_SECS: f32 = 0.1;

/// Channel fader curve: approximately logarithmic, silent at 0 and unity at 1.
pub fn fader_gain(value: f32) -> f32 {
    let v = value.clamp(0.0, 1.0);
    v * v
}

/// Full-at-centre crossfader: returns `(gain_a, gain_b)` for `x` in [0, 1].
pub fn crossfader_gains(x: f32) -> (f32, f32) {
    let x = x.clamp(0.0, 1.0);
    ((2.0 * (1.0 - x)).min(1.0), (2.0 * x).min(1.0))
}

/// A stereo peak limiter with instant attack and exponential release. The output never
/// exceeds [`LIMITER_CEILING`].
#[derive(Debug, Clone)]
pub struct Limiter {
    gain: f32,
    release_coef: f32,
}

impl Limiter {
    pub fn new(sample_rate: u32) -> Self {
        let mut l = Self {
            gain: 1.0,
            release_coef: 0.0,
        };
        l.set_sample_rate(sample_rate);
        l
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.release_coef = 1.0 - (-1.0 / (LIMITER_RELEASE_SECS * sample_rate.max(1) as f32)).exp();
    }

    #[inline]
    pub fn process(&mut self, l: &mut f32, r: &mut f32) {
        // Release towards unity.
        self.gain += (1.0 - self.gain) * self.release_coef;
        let peak = l.abs().max(r.abs());
        if peak * self.gain > LIMITER_CEILING {
            self.gain = LIMITER_CEILING / peak;
        }
        *l *= self.gain;
        *r *= self.gain;
        // Guard against NaN/inf from upstream.
        if !l.is_finite() {
            *l = 0.0;
        }
        if !r.is_finite() {
            *r = 0.0;
        }
    }
}

/// The two-deck mixer.
#[derive(Debug, Clone)]
pub struct Mixer {
    pub state: MixerState,
    master_limiter: Limiter,
    headphone_limiter: Limiter,
}

/// Gains that hold for a whole block (see [`Mixer::block_gains`]), so the per-frame mix
/// is only multiplies and adds.
#[derive(Debug, Clone, Copy)]
pub struct BlockGains {
    /// Per-deck gain into the master (channel fader × crossfader × master level).
    master: [f32; DECK_COUNT],
    /// Per-deck gain into the headphones: the headphone level for cued decks, else 0.
    cue: [f32; DECK_COUNT],
    /// Gain of the master mix into the headphones: the headphone level when no deck is
    /// cued, else 0.
    headphone_master: f32,
}

/// One stereo frame from each deck.
pub type DeckFrames = [(f32, f32); DECK_COUNT];

impl Mixer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            state: MixerState::default(),
            master_limiter: Limiter::new(sample_rate),
            headphone_limiter: Limiter::new(sample_rate),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.master_limiter.set_sample_rate(sample_rate);
        self.headphone_limiter.set_sample_rate(sample_rate);
    }

    /// The gains for a block, worked out once from the current fader positions.
    pub fn block_gains(&self) -> BlockGains {
        let (xa, xb) = crossfader_gains(self.state.crossfader);
        let master = fader_gain(self.state.master_level);
        let headphone = fader_gain(self.state.headphone_level);
        BlockGains {
            master: [
                fader_gain(self.state.channel_fader[0]) * xa * master,
                fader_gain(self.state.channel_fader[1]) * xb * master,
            ],
            cue: self
                .state
                .headphone_cue
                .map(|on| if on { headphone } else { 0.0 }),
            headphone_master: if self.state.headphone_cue.contains(&true) {
                0.0
            } else {
                headphone
            },
        }
    }

    /// Mix one frame. Returns `(master, headphones)` stereo pairs, both limited.
    #[inline]
    pub fn mix(&mut self, decks: DeckFrames, gains: &BlockGains) -> ((f32, f32), (f32, f32)) {
        let mut ml = 0.0;
        let mut mr = 0.0;
        let mut hl = 0.0;
        let mut hr = 0.0;
        for ((d, g), cue) in decks.iter().zip(gains.master).zip(gains.cue) {
            ml += d.0 * g;
            mr += d.1 * g;
            hl += d.0 * cue;
            hr += d.1 * cue;
        }
        // With no deck cued, the headphones carry the master mix (before its limiter).
        hl += ml * gains.headphone_master;
        hr += mr * gains.headphone_master;

        self.master_limiter.process(&mut ml, &mut mr);
        self.headphone_limiter.process(&mut hl, &mut hr);
        ((ml, mr), (hl, hr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOUD: (f32, f32) = (1.0, 1.0);
    const SILENT: (f32, f32) = (0.0, 0.0);

    fn mixer() -> Mixer {
        Mixer::new(48_000)
    }

    fn master_of(m: &mut Mixer, decks: DeckFrames) -> (f32, f32) {
        let g = m.block_gains();
        m.mix(decks, &g).0
    }

    #[test]
    fn fader_down_silences_deck() {
        let mut m = mixer();
        m.state.channel_fader[0] = 0.0;
        let (l, r) = master_of(&mut m, [(0.5, 0.5), SILENT]);
        assert_eq!((l, r), (0.0, 0.0));
    }

    #[test]
    fn fader_curve_endpoints() {
        assert_eq!(fader_gain(0.0), 0.0);
        assert_eq!(fader_gain(1.0), 1.0);
        assert!(fader_gain(0.5) < 0.5);
    }

    #[test]
    fn crossfader_left_plays_only_a() {
        let mut m = mixer();
        m.state.crossfader = 0.0;
        let (l, _) = master_of(&mut m, [(0.25, 0.25), (0.5, 0.5)]);
        assert!((l - 0.25).abs() < 1e-6);
        m.state.crossfader = 1.0;
        let (l, _) = master_of(&mut m, [(0.25, 0.25), (0.5, 0.5)]);
        assert!((l - 0.5).abs() < 1e-6);
    }

    #[test]
    fn crossfader_centre_plays_both_at_fader_level() {
        assert_eq!(crossfader_gains(0.5), (1.0, 1.0));
        let mut m = mixer();
        m.state.channel_fader = [1.0, 0.5];
        let (l, _) = master_of(&mut m, [(0.25, 0.25), (0.25, 0.25)]);
        assert!((l - (0.25 + 0.25 * 0.25)).abs() < 1e-6);
    }

    #[test]
    fn crossfader_applies_after_channel_fader() {
        let mut m = mixer();
        m.state.channel_fader = [0.0, 1.0];
        m.state.crossfader = 0.0;
        let (l, _) = master_of(&mut m, [LOUD, LOUD]);
        assert_eq!(l, 0.0);
    }

    #[test]
    fn two_full_scale_signals_never_exceed_ceiling() {
        let mut m = mixer();
        for i in 0..48_000 {
            let v = if i % 2 == 0 { 1.0 } else { -1.0 };
            let ((l, r), (hl, hr)) = {
                let g = m.block_gains();
                m.mix([(v, v), (v, v)], &g)
            };
            for s in [l, r, hl, hr] {
                assert!(s.abs() <= LIMITER_CEILING + 1e-6, "sample {s} at {i}");
            }
        }
    }

    #[test]
    fn limiter_releases_to_unity() {
        let mut lim = Limiter::new(1_000);
        let (mut l, mut r) = (4.0, 4.0);
        lim.process(&mut l, &mut r);
        for _ in 0..2_000 {
            let (mut l, mut r) = (0.1, 0.1);
            lim.process(&mut l, &mut r);
        }
        let (mut l, mut r) = (0.5, 0.5);
        lim.process(&mut l, &mut r);
        assert!((l - 0.5).abs() < 1e-3);
    }

    #[test]
    fn headphone_cue_is_pre_fader() {
        let mut m = mixer();
        m.state.channel_fader[1] = 0.0;
        m.state.headphone_cue[1] = true;
        m.state.headphone_level = 1.0;
        let g = m.block_gains();
        let ((ml, _), (hl, _)) = m.mix([SILENT, (0.5, 0.5)], &g);
        assert_eq!(ml, 0.0, "deck B not in master");
        assert!((hl - 0.5).abs() < 1e-6, "deck B in headphones");
    }

    #[test]
    fn headphones_carry_master_without_cue() {
        let mut m = mixer();
        m.state.headphone_level = 1.0;
        let g = m.block_gains();
        let ((ml, _), (hl, _)) = m.mix([(0.3, 0.3), (0.2, 0.2)], &g);
        assert!((ml - hl).abs() < 1e-6);
        assert!((ml - 0.5).abs() < 1e-6);
    }

    #[test]
    fn master_level_scales_output() {
        let mut m = mixer();
        m.state.master_level = 0.0;
        let (l, _) = master_of(&mut m, [LOUD, LOUD]);
        assert_eq!(l, 0.0);
    }
}
