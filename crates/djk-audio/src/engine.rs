//! The real-time engine: drains command queues, renders decks through the mixer, routes the
//! buses to output channels and publishes a state snapshot.
//!
//! [`Engine::process`] is called from the audio callback and must never block, allocate,
//! free memory or do I/O. Allocation-freedom is enforced by `tests/no_alloc.rs`.

use std::time::Instant;

use djk_core::{Command, DECK_COUNT, EngineState, LoadRequest, TrackBuffer};
use rtrb::{Consumer, Producer, RingBuffer};
use triple_buffer::{Input, Output};

use crate::deck::Deck;
use crate::eq::Eq;
use crate::mixer::Mixer;

/// Frames rendered per internal chunk; larger callbacks are processed in several chunks.
pub const CHUNK_FRAMES: usize = 1024;
const COMMAND_QUEUE: usize = 1024;
const LOAD_QUEUE: usize = 4;
const RETIRE_QUEUE: usize = 16;

/// The non-real-time side of the engine, held by the app.
pub struct EngineHandles {
    /// Commands from the UI thread.
    pub ui: Producer<Command>,
    /// Commands from the controller (MIDI) thread.
    pub controller: Option<Producer<Command>>,
    /// Track loads.
    pub loads: Producer<LoadRequest>,
    /// Latest engine snapshot.
    pub state: Output<EngineState>,
    /// Buffers the engine no longer uses; drop them on a non-audio thread.
    pub retired: Consumer<TrackBuffer>,
}

impl EngineHandles {
    /// Drop buffers the engine has finished with. Call regularly from the UI thread.
    pub fn drop_retired(&mut self) -> usize {
        let mut n = 0;
        while self.retired.pop().is_ok() {
            n += 1;
        }
        n
    }

    /// Send a UI command. Returns false if the queue is full.
    pub fn send(&mut self, cmd: Command) -> bool {
        self.ui.push(cmd).is_ok()
    }

    /// Queue a track load, ordered with the other UI commands. Returns the request back if
    /// a queue is full.
    pub fn load(&mut self, req: LoadRequest) -> Result<(), LoadRequest> {
        if self.ui.is_full() {
            return Err(req);
        }
        let deck = req.deck;
        self.loads.push(req).map_err(|rtrb::PushError::Full(r)| r)?;
        // Cannot fail: checked above and this is the only producer.
        let _ = self.ui.push(Command::Load(deck));
        Ok(())
    }
}

/// The audio-thread side of the engine.
pub struct Engine {
    decks: [Deck; DECK_COUNT],
    mixer: Mixer,
    ui_commands: Consumer<Command>,
    controller_commands: Consumer<Command>,
    loads: Consumer<LoadRequest>,
    retire: Producer<TrackBuffer>,
    state_out: Input<EngineState>,
    scratch: [[Vec<f32>; 2]; DECK_COUNT],
    device_rate: u32,
    output_channels: u16,
    output_latency: f64,
    overruns: u64,
    refused_loads: u64,
    frames_rendered: u64,
    channel_peak: [f32; DECK_COUNT],
    master_peak: [f32; 2],
    /// Per-deck 3-band EQ, applied before the channel fader.
    eqs: [Eq; DECK_COUNT],
}

impl Engine {
    /// Create an engine for the given output rate and channel count, plus its handles.
    pub fn new(device_rate: u32, output_channels: u16) -> (Engine, EngineHandles) {
        let (ui_tx, ui_rx) = RingBuffer::new(COMMAND_QUEUE);
        let (ctl_tx, ctl_rx) = RingBuffer::new(COMMAND_QUEUE);
        let (load_tx, load_rx) = RingBuffer::new(LOAD_QUEUE);
        let (retire_tx, retire_rx) = RingBuffer::new(RETIRE_QUEUE);
        let (state_in, state_out) = triple_buffer::triple_buffer(&EngineState::default());
        let scratch = || [vec![0.0; CHUNK_FRAMES], vec![0.0; CHUNK_FRAMES]];
        let mut engine = Engine {
            decks: [Deck::new(), Deck::new()],
            mixer: Mixer::new(device_rate),
            ui_commands: ui_rx,
            controller_commands: ctl_rx,
            loads: load_rx,
            retire: retire_tx,
            state_out: state_in,
            scratch: [scratch(), scratch()],
            device_rate,
            output_channels,
            output_latency: 0.0,
            overruns: 0,
            refused_loads: 0,
            frames_rendered: 0,
            channel_peak: [0.0; DECK_COUNT],
            master_peak: [0.0; 2],
            eqs: [Eq::new(device_rate), Eq::new(device_rate)],
        };
        engine.publish(None);
        let handles = EngineHandles {
            ui: ui_tx,
            controller: Some(ctl_tx),
            loads: load_tx,
            state: state_out,
            retired: retire_rx,
        };
        (engine, handles)
    }

    /// Change the output format (e.g. after switching device). Not real-time; call while
    /// no stream is running this engine.
    pub fn set_output(&mut self, device_rate: u32, output_channels: u16) {
        self.device_rate = device_rate;
        self.output_channels = output_channels;
        self.mixer.set_sample_rate(device_rate);
        for eq in &mut self.eqs {
            eq.set_sample_rate(device_rate);
        }
        self.publish(None);
    }

    pub fn device_rate(&self) -> u32 {
        self.device_rate
    }

    /// Set the measured output latency (seconds) reported in snapshots.
    pub fn set_output_latency(&mut self, secs: f64) {
        self.output_latency = secs;
    }

    /// Record a missed audio deadline.
    pub fn note_overrun(&mut self) {
        self.overruns += 1;
    }

    /// Render one callback's worth of interleaved output.
    ///
    /// Master goes to channels 1–2 and headphones to 3–4 when `channels >= 4`; with fewer
    /// channels only the master is output (mono devices get the mono sum).
    pub fn process(&mut self, out: &mut [f32], channels: usize, now: Option<Instant>) {
        self.drain_commands();

        let channels = channels.max(1);
        let total_frames = out.len() / channels;
        let mut done = 0;
        self.channel_peak = [0.0; DECK_COUNT];
        self.master_peak = [0.0; 2];
        while done < total_frames {
            let n = (total_frames - done).min(CHUNK_FRAMES);
            for (((deck, buf), peak), eq) in self
                .decks
                .iter_mut()
                .zip(self.scratch.iter_mut())
                .zip(self.channel_peak.iter_mut())
                .zip(self.eqs.iter_mut())
            {
                let [l, r] = buf;
                deck.render(self.device_rate, &mut l[..n], &mut r[..n]);
                // EQ before the channel fader, so the channel meter is post-EQ.
                eq.process(&mut l[..n], &mut r[..n]);
                for (a, b) in l[..n].iter().zip(&r[..n]) {
                    *peak = peak.max(a.abs()).max(b.abs());
                }
            }
            let gains = self.mixer.master_gains();
            let chunk = &mut out[done * channels..(done + n) * channels];
            for (i, frame) in chunk.chunks_exact_mut(channels).enumerate() {
                let decks = [
                    (self.scratch[0][0][i], self.scratch[0][1][i]),
                    (self.scratch[1][0][i], self.scratch[1][1][i]),
                ];
                let ((ml, mr), (hl, hr)) = self.mixer.mix(decks, &gains);
                self.master_peak[0] = self.master_peak[0].max(ml.abs());
                self.master_peak[1] = self.master_peak[1].max(mr.abs());
                match channels {
                    1 => frame[0] = 0.5 * (ml + mr),
                    2 | 3 => {
                        frame[0] = ml;
                        frame[1] = mr;
                        frame[2..].fill(0.0);
                    }
                    _ => {
                        frame[0] = ml;
                        frame[1] = mr;
                        frame[2] = hl;
                        frame[3] = hr;
                        frame[4..].fill(0.0);
                    }
                }
            }
            done += n;
        }
        // Any trailing partial frame.
        out[total_frames * channels..].fill(0.0);
        self.frames_rendered += total_frames as u64;
        self.publish(now);
    }

    fn drain_commands(&mut self) {
        while let Ok(&cmd) = self.ui_commands.peek() {
            // Only take a load when there is room to retire the outgoing buffer (or the
            // refused incoming one), so nothing is ever freed on this thread. Otherwise leave
            // it, and everything after it, for the next block.
            if matches!(cmd, Command::Load(_)) && self.retire.slots() < 1 {
                break;
            }
            let _ = self.ui_commands.pop();
            self.apply(cmd);
        }
        while let Ok(cmd) = self.controller_commands.pop() {
            // Loads only come from the UI queue.
            if !matches!(cmd, Command::Load(_)) {
                self.apply(cmd);
            }
        }
    }

    fn take_load(&mut self) {
        let Ok(req) = self.loads.pop() else { return };
        let deck = &mut self.decks[req.deck.index()];
        let retired = if deck.is_playing() {
            self.refused_loads += 1;
            Some(req.buffer)
        } else {
            deck.load(req.buffer, req.token)
        };
        if let Some(buf) = retired {
            // Room was checked before popping the Load command.
            let _ = self.retire.push(buf);
        }
    }

    /// Apply one command immediately.
    fn apply(&mut self, cmd: Command) {
        let mix = &mut self.mixer.state;
        match cmd {
            Command::Load(_) => self.take_load(),
            Command::Play(d) => self.decks[d.index()].play(),
            Command::Pause(d) => self.decks[d.index()].pause(),
            Command::TogglePlay(d) => self.decks[d.index()].toggle_play(),
            Command::CuePress(d) => self.decks[d.index()].cue_press(),
            Command::CueRelease(d) => self.decks[d.index()].cue_release(),
            Command::Seek { deck, frame } => self.decks[deck.index()].seek(frame),
            Command::SetPitch { deck, pitch } => self.decks[deck.index()].set_pitch(pitch),
            Command::Jog { deck, ticks } => self.decks[deck.index()].jog(ticks),
            Command::SetChannelFader { deck, value } => {
                mix.channel_fader[deck.index()] = value.clamp(0.0, 1.0);
            }
            Command::SetCrossfader(x) => mix.crossfader = x.clamp(0.0, 1.0),
            Command::SetMasterLevel(v) => mix.master_level = v.clamp(0.0, 1.0),
            Command::SetHeadphoneLevel(v) => mix.headphone_level = v.clamp(0.0, 1.0),
            Command::SetHeadphoneCue { deck, enabled } => {
                mix.headphone_cue[deck.index()] = enabled;
            }
            Command::ToggleHeadphoneCue(d) => {
                mix.headphone_cue[d.index()] = !mix.headphone_cue[d.index()];
            }
            Command::SetEq { deck, band, value } => {
                mix.eq[deck.index()][band.index()] = value.clamp(0.0, 1.0);
                self.eqs[deck.index()].set_knobs(mix.eq[deck.index()]);
            }
        }
    }

    /// Build the current snapshot.
    pub fn snapshot(&self, now: Option<Instant>) -> EngineState {
        EngineState {
            decks: [self.decks[0].state(), self.decks[1].state()],
            mixer: self.mixer.state,
            timestamp: now,
            output_latency: self.output_latency,
            device_rate: self.device_rate,
            output_channels: self.output_channels,
            overruns: self.overruns,
            refused_loads: self.refused_loads,
            frames_rendered: self.frames_rendered,
            channel_peak: self.channel_peak,
            master_peak: self.master_peak,
        }
    }

    fn publish(&mut self, now: Option<Instant>) {
        let snap = self.snapshot(now);
        self.state_out.write(snap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djk_core::DeckId;
    use std::sync::Arc;

    fn tone(rate: u32, secs: f32, amp: f32) -> TrackBuffer {
        let n = (rate as f32 * secs) as usize;
        let s: Vec<f32> = (0..n)
            .flat_map(|i| {
                let v = (i as f32 * 0.05).sin() * amp;
                [v, v]
            })
            .collect();
        TrackBuffer {
            samples: Arc::from(s),
            sample_rate: rate,
        }
    }

    fn load(h: &mut EngineHandles, deck: DeckId, buf: TrackBuffer, token: u64) {
        h.load(LoadRequest {
            deck,
            buffer: buf,
            token,
        })
        .unwrap();
    }

    #[test]
    fn routes_master_and_headphones_on_four_channels() {
        let (mut e, mut h) = Engine::new(48_000, 4);
        load(&mut h, DeckId::B, tone(48_000, 1.0, 0.5), 7);
        h.send(Command::SetChannelFader {
            deck: DeckId::B,
            value: 0.0,
        });
        h.send(Command::SetHeadphoneCue {
            deck: DeckId::B,
            enabled: true,
        });
        h.send(Command::SetHeadphoneLevel(1.0));
        h.send(Command::Play(DeckId::B));
        let mut out = vec![0.0; 512 * 4];
        e.process(&mut out, 4, None);
        let master_energy: f32 = out.chunks(4).map(|f| f[0].abs() + f[1].abs()).sum();
        let hp_energy: f32 = out.chunks(4).map(|f| f[2].abs() + f[3].abs()).sum();
        assert_eq!(master_energy, 0.0);
        assert!(hp_energy > 1.0);
        let s = *h.state.read();
        assert!(s.decks[1].playing);
        assert_eq!(s.decks[1].track_token, 7);
        assert!(s.headphones_available());
    }

    #[test]
    fn eq_kill_silences_a_band_through_the_engine() {
        let (mut e, mut h) = Engine::new(48_000, 2);
        // A 60 Hz tone: all low band.
        let n = 48_000;
        let s: Vec<f32> = (0..n)
            .flat_map(|i| {
                let v = (2.0 * std::f32::consts::PI * 60.0 * i as f32 / 48_000.0).sin() * 0.5;
                [v, v]
            })
            .collect();
        load(
            &mut h,
            DeckId::A,
            TrackBuffer {
                samples: Arc::from(s),
                sample_rate: 48_000,
            },
            1,
        );
        h.send(Command::Play(DeckId::A));
        let mut out = vec![0.0; 2048 * 2];
        e.process(&mut out, 2, None);
        assert!(h.state.read().master_peak[0] > 0.3);
        h.send(Command::SetEq {
            deck: DeckId::A,
            band: djk_core::EqBand::Low,
            value: 0.0,
        });
        for _ in 0..4 {
            e.process(&mut out, 2, None);
        }
        let s = *h.state.read();
        assert_eq!(s.mixer.eq[0][0], 0.0);
        assert!(s.master_peak[0] < 0.01, "{:?}", s.master_peak);
    }

    #[test]
    fn reports_pre_fader_channel_peaks_and_master_peaks() {
        let (mut e, mut h) = Engine::new(48_000, 2);
        load(&mut h, DeckId::A, tone(48_000, 1.0, 0.5), 1);
        // Channel fader down: the channel meter still shows the signal, the master
        // does not.
        h.send(Command::SetChannelFader {
            deck: DeckId::A,
            value: 0.0,
        });
        h.send(Command::Play(DeckId::A));
        let mut out = vec![0.0; 1024 * 2];
        // Warm up first: the EQ's crossover filters overshoot briefly on the tone's abrupt
        // start, and this test is about the steady level.
        e.process(&mut out, 2, None);
        e.process(&mut out, 2, None);
        let s = *h.state.read();
        assert!(
            (0.45..=0.52).contains(&s.channel_peak[0]),
            "{}",
            s.channel_peak[0]
        );
        assert_eq!(s.channel_peak[1], 0.0);
        assert_eq!(s.master_peak, [0.0, 0.0]);

        h.send(Command::SetChannelFader {
            deck: DeckId::A,
            value: 1.0,
        });
        e.process(&mut out, 2, None);
        let s = *h.state.read();
        assert!(
            s.master_peak[0] > 0.4 && s.master_peak[0] <= 0.52,
            "{:?}",
            s.master_peak
        );
    }

    #[test]
    fn stereo_device_gets_master_only() {
        let (mut e, mut h) = Engine::new(48_000, 2);
        load(&mut h, DeckId::A, tone(48_000, 1.0, 0.5), 1);
        h.send(Command::Play(DeckId::A));
        let mut out = vec![0.0; 256 * 2];
        e.process(&mut out, 2, None);
        assert!(out.iter().any(|&v| v != 0.0));
        assert!(!h.state.read().headphones_available());
    }

    #[test]
    fn load_onto_playing_deck_is_refused_and_buffers_retired() {
        let (mut e, mut h) = Engine::new(48_000, 2);
        load(&mut h, DeckId::A, tone(48_000, 1.0, 0.5), 1);
        h.send(Command::Play(DeckId::A));
        let mut out = vec![0.0; 64 * 2];
        e.process(&mut out, 2, None);
        load(&mut h, DeckId::A, tone(48_000, 1.0, 0.5), 2);
        e.process(&mut out, 2, None);
        let s = *h.state.read();
        assert!(s.decks[0].playing);
        assert_eq!(s.decks[0].track_token, 1);
        assert_eq!(s.refused_loads, 1);
        assert_eq!(h.drop_retired(), 1);

        // Pause, then a load replaces the buffer and retires the old one.
        h.send(Command::Pause(DeckId::A));
        load(&mut h, DeckId::A, tone(48_000, 1.0, 0.5), 3);
        e.process(&mut out, 2, None);
        let s = *h.state.read();
        assert_eq!(s.decks[0].track_token, 3);
        assert_eq!(s.decks[0].position, 0.0);
        assert_eq!(h.drop_retired(), 1);
    }

    #[test]
    fn controller_queue_is_drained() {
        let (mut e, mut h) = Engine::new(48_000, 2);
        let mut ctl = h.controller.take().unwrap();
        ctl.push(Command::SetCrossfader(1.0)).unwrap();
        let mut out = vec![0.0; 32];
        e.process(&mut out, 2, None);
        assert_eq!(h.state.read().mixer.crossfader, 1.0);
    }

    #[test]
    fn large_callbacks_are_chunked() {
        let (mut e, mut h) = Engine::new(48_000, 2);
        load(&mut h, DeckId::A, tone(48_000, 1.0, 0.5), 1);
        h.send(Command::Play(DeckId::A));
        let mut out = vec![0.0; (CHUNK_FRAMES * 3 + 17) * 2];
        e.process(&mut out, 2, None);
        let s = *h.state.read();
        assert_eq!(s.decks[0].position, (CHUNK_FRAMES * 3 + 17) as f64);
        assert!(out[out.len() - 2] != 0.0);
    }
}
