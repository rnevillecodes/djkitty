# Design

## Context

Each deck's signal path in `Engine::process` (`crates/djk-audio/src/engine.rs`) is: `Deck::render` into a per-deck scratch buffer, then `Eq::process` on that buffer, then the channel peak meter, then `Mixer::mix` (channel fader, crossfader, master, limiters, headphone cue). The EQ (`eq.rs`) is a Linkwitz-Riley isolator built from transposed direct form II biquads with 10 ms per-sample gain smoothing, set by `Command::SetEq` and stored in `MixerState::eq`.

On the controller side, the EQ knobs are `Control::Fader(FaderTarget::Eq(..), Msb/Lsb)` entries in the FLX2 table (`mapping.rs`), with soft takeover in `takeover.rs` (`FaderTarget::COUNT` slots) and the initial position SysEx applying each control's first report directly.

On screen, `mixer_strip` (`ui.rs`) draws three 30 px EQ knobs per channel in an outer column, spaced at `(fader_h + 14) / 3`. With the deck row fixed at 270 px, `fader_h` is about 179 px, so the step is about 64 px for a knob plus label needing about 42 px. `widgets::knob` already supports a bipolar mode (arc from 12 o'clock, gentle snap within ±0.015 of the centre while dragging) and double-click or Ctrl+click reset.

## Goals / Non-Goals

**Goals:**
- A per-deck bipolar LPF/HPF filter that is bit-transparent at the centre, sweeps musically, and never clicks.
- Controller CFX knob support with the same takeover behaviour as the EQ.
- A FILTER knob that fits in the existing mixer column without resizing anything.

**Non-Goals:**
- Smart CFX echo or reverb, resonance control, filter LFOs or anything tempo-synced.
- Saving filter positions.
- Mapping the CFX (Smart CFX) button.

## Decisions

### 1. Value model: one 0–1 position, 0.5 = off

The filter is a single `f32` per deck in `MixerState` (`filter: [f32; DECK_COUNT]`, default 0.5), set by `Command::SetFilter { deck, value }`. This mirrors the EQ (same 0–1 range, same takeover, same knob widget), keeps `Command` within 24 bytes, and makes the hardware centre detent the filter's off position.

The mapping from position to filter, with `d = value − 0.5`:
- `|d| ≤ DEAD_ZONE` (0.02, just wider than the knob's ±0.015 snap): bypassed.
- `d < 0`: low-pass, `u = (|d| − DEAD_ZONE) / (0.5 − DEAD_ZONE)` in 0..1, cutoff `20 kHz × (100 / 20 000)^u` (log sweep 20 kHz → 100 Hz).
- `d > 0`: high-pass, cutoff `20 Hz × (8 000 / 20)^u` (log sweep 20 Hz → 8 kHz).

The cutoff is also clamped below about 0.45 × the device sample rate so the filter stays stable at 44.1 kHz.

*Alternative:* separate LPF and HPF amounts. Rejected: the hardware is one knob, and one value keeps takeover and the UI identical to the EQ.

### 2. DSP: a state-variable filter with smoothed cutoff, cross-faded bypass

`djk-audio/src/filter.rs` holds a per-deck stereo filter using the Cytomic/Simper trapezoidal state-variable filter (SVF), which gives low-pass and high-pass outputs from the same state, stays stable under fast cutoff modulation (unlike recomputing biquad coefficients per sample), and costs a few multiplies per sample.

- **12 dB/oct, gentle resonance:** the SVF is 2nd order. Q is fixed at about 0.9 (a peak of roughly +1 dB near the cutoff), enough to sound like a DJ filter without ringing or raising the level noticeably.
- **Smoothing:** the target cutoff is smoothed per sample in the log-frequency domain with the same ~10 ms time constant as the EQ, so a fast knob move sweeps rather than steps. Coefficients are recomputed from the smoothed cutoff every 16 samples (a `tan` per update), which is inaudible at these rates and cheap.
- **Mode changes and bypass:** a per-deck wet mix (0 = dry, 1 = filtered) is also smoothed. Entering the dead zone fades the wet mix to 0 and only then marks the filter bypassed, so the output becomes exactly the dry input (bit-transparent, no phase shift) without a click. Crossing from LPF to HPF passes through the dead zone, so the mode switch always happens while the wet mix is at 0 and the filter state is reset there. While bypassed and the target is centred, the filter does no work at all.
- **Real-time safety:** fixed-size state, no allocation, locking or I/O. The `no_alloc` test covers it, and non-finite samples are guarded like the EQ.

*Alternative:* biquad LPF/HPF with per-block coefficient updates. Rejected: biquads modulated quickly can zipper or blow up, and the SVF handles both modes from one structure.

### 3. Placement in the engine

In `Engine::process`, each deck's chunk goes `render → eq.process → filter.process → peak meter`. The filter is pre-fader like the EQ, so the channel meter, the master and the headphone cue all hear it, matching the spec and the hardware. `Engine` gains `filters: [Filter; DECK_COUNT]` with `set_sample_rate` alongside the EQs.

### 4. Controller mapping and takeover

`FaderTarget` gains `Filter(DeckId)` (`COUNT` becomes 13), so the CFX knobs get the same soft takeover and first-report-applies behaviour as the EQ knobs, and `ui_fader` records on-screen changes for pickup. The FLX2 table gains the CFX knobs as 14-bit MSB/LSB CCs. Until recorded, the numbers follow the DDJ-400/FLX4 convention (mixer channel 7: CC `0x17`/`0x37` for channel 1 and `0x18`/`0x38` for channel 2), which the first task verifies. The CFX button stays absent from the table, so it falls under "unmapped controls are ignored".

### 5. On-screen knob

In `mixer_strip`, the EQ loop becomes four rows: HI, MID, LOW, FILTER, with the step changed from `/ 3` to `/ 4` (about 48 px against 42 px needed). The FILTER knob uses `widgets::knob` in bipolar mode with a neutral accent (`theme::TEXT`) since it is not an EQ band. Its label uses `theme::TEXT` while the filter is active and `theme::TEXT_DIM` when centred. The tooltip reads "Deck A filter: LPF 800 Hz", "HPF 2.1 kHz" or "off", from a shared `filter::describe(value)` so the UI and DSP agree on the cutoff. Everything else in the card stays where it is, checked with the debug layout dump at 1280×760 and 960×600.

The label is FILTER rather than the hardware's CFX, because in djkitty that is all it does.

## Risks / Trade-offs

- [The FLX2's CFX knob messages are unknown] → The first task records them with `--midi-monitor`. The mapping is data, so only the table changes.
- [Tighter EQ column] → About 6 px spare per row. If labels touch at the minimum window size, the knob diameter drops to 28 px for all four rows rather than resizing the card.
- [The filter direction convention] → Left = low-pass follows rekordbox and Mixxx. Serato's FLX2 guide words it the other way, so the hardware labelling is checked in the first task. Flipping is one sign in the mapping.
- [The resonant peak adds a little level] → Q ≈ 0.9 keeps it near +1 dB, and the master limiter is downstream.
- [Archive order] → `controller-input` only exists in `add-v0-1-walking-skeleton`, so that change must be archived first.

## Migration Plan

No data or settings changes. Filters start centred, so existing mixes sound the same until the knob is moved.

## Open Questions

- What MIDI messages do the FLX2 CFX knobs send, and does the FLX2 centre detent land exactly on the MIDI centre value? (If not, the dead zone already absorbs a small offset.)
