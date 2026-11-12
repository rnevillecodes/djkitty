# Proposal

## Why

A filter sweep is the transition tool a learner reaches for right after the EQ, and the DDJ-FLX2 has a dedicated CFX knob under each channel's EQ that does nothing in djkitty today. A plain filter (no tempo-synced effects) fits djkitty's manual, learn-by-ear approach: what you hear is exactly what your hand does.

## What Changes

- **Per-channel filter:** one bipolar FILTER knob per deck. Turning left from the centre sweeps a low-pass filter down (the track gets muffled until only bass is left), turning right sweeps a high-pass filter up (the bass goes and the track thins out). At the centre the filter is fully bypassed, so a centred knob never changes the sound.
- **Sound:** 12 dB/octave with gentle resonance. The cutoff moves on a log scale (low-pass from about 20 kHz down to about 100 Hz, high-pass from about 20 Hz up to about 8 kHz) and is smoothed so sweeps never click or zip.
- **Signal path:** the filter sits after the EQ and before the channel fader, like the EQ, so the headphone cue and the channel meter hear it too.
- **On screen:** a fourth knob under LOW in each channel's EQ column, labelled FILTER. It uses a neutral colour with an arc that grows from the centre, its label lights up while the filter is active, its tooltip shows the filter type and cutoff (for example "LPF 800 Hz"), and double-click or Ctrl+click resets it to the centre. The mixer card and the rest of the layout keep their size.
- **Controller:** the DDJ-FLX2's CFX knob for each channel drives the filter, with the same soft takeover and initial position report as the EQ knobs. The CFX (Smart CFX) button stays unmapped.
- The filter position is not saved; like the EQ it is a live mixing control.
- **Out of scope:** Smart CFX echo and reverb, filter resonance control, and any tempo-synced effect.

## Capabilities

### New Capabilities

- `channel-filter`: the per-deck bipolar low-pass/high-pass filter: its position in the signal path, bypass at the centre, cutoff range, smoothing, and its on-screen control.

### Modified Capabilities

- `controller-input`: adds the DDJ-FLX2 CFX knobs to the mapped mixer controls, with soft takeover. (This capability is still being introduced by `add-v0-1-walking-skeleton`, so the delta only ADDs a requirement.)

## Impact

- `djk-core`: a new `SetFilter { deck, value }` command and a per-deck filter position in `MixerState`. `Command` must stay within its 24-byte limit.
- `djk-audio`: a new `filter.rs` (bipolar state-variable filter with smoothed cutoff), run per deck right after the EQ in `Engine::process`. Real-time safe: no allocation, the `no_alloc` test must still pass.
- `djk-controller`: a `FaderTarget::Filter(deck)` with soft takeover, and the CFX knob CCs in the FLX2 mapping (provisional FLX4 numbers until recorded on the hardware).
- `djk-app`: the FILTER knob in `mixer_strip` (EQ step divided by 4 instead of 3), its tooltip and active label.
- `docs/controllers/ddj-flx2.md`: CFX knob rows to verify on the hardware.
- No library or file-format changes.
