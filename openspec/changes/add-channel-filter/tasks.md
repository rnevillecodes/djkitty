# Tasks

## 1. Hardware discovery

- [ ] 1.1 With `--midi-monitor`, record both DDJ-FLX2 CFX knobs (MSB and LSB, full travel and the centre detent value) and the CFX button, and check which direction the hardware labels as low-pass. Verify by adding CFX rows to the incoming messages table in `docs/controllers/ddj-flx2.md`, including whether the detent lands on the MIDI centre

## 2. Core and engine

- [ ] 2.1 Add `filter: [f32; DECK_COUNT]` (default 0.5) to `MixerState` and `SetFilter { deck, value }` to `Command`, clamped to 0–1 when applied. Verify `cargo build --workspace` succeeds and the `Command` size test still passes
- [ ] 2.2 Implement `djk-audio/src/filter.rs` (design Decision 2): position-to-cutoff mapping with the dead zone, a stereo trapezoidal SVF with fixed Q ≈ 0.9, log-domain cutoff smoothing, smoothed wet mix with exact bypass, cutoff clamped below Nyquist, non-finite guard, and a `describe(value)` helper. Verify with unit tests that a centred (and in-dead-zone) filter is bit-identical to its input, that fully left attenuates a 5 kHz sine by more than 30 dB while passing 60 Hz within 1 dB, that fully right attenuates a 60 Hz sine by more than 30 dB while passing 10 kHz within 1 dB, that the cutoff mapping is logarithmic (equal steps give equal frequency ratios), that a full left-to-right sweep in one block has no sample-to-sample jump above the input's own maximum step, and that the peak gain at any setting stays under +3 dB
- [ ] 2.3 Run each deck's filter after its EQ in `Engine::process` and apply `SetFilter` (design Decision 3), including `set_sample_rate`. Verify with an engine test that a fully right filter on a cued deck reaches the headphone output high-passed, and that `no_alloc` still passes

## 3. Controller

- [ ] 3.1 Add `FaderTarget::Filter(DeckId)` (bumping `COUNT`) with soft takeover and first-report-applies, the CFX knob CCs from task 1.1 in the FLX2 table, and the `SetFilter` translation (design Decision 4). Verify with unit tests feeding the recorded bytes for every `controller-input` delta scenario, including takeover after an on-screen reset and that the CFX button yields nothing

## 4. App

- [ ] 4.1 Add the FILTER knob to `mixer_strip` as a fourth row under LOW (step `/ 4`), bipolar with a neutral accent, highlighted label while active, tooltip from `filter::describe`, sending `SetFilter` and recording `ui_fader` (design Decision 5). Verify every `channel-filter` "on screen" scenario with no controller connected, and compare the debug layout dump at 1280×760 and 960×600 with the current one to confirm only the EQ knob rows moved and nothing overlaps
- [ ] 4.2 Update the README mixer bullet and the FLX2 control table in `docs/controllers/ddj-flx2.md` with the filter. Verify both describe it as built

## 5. End-to-end check

- [ ] 5.1 On the DDJ-FLX2, play two tracks and do a filter transition: sweep the outgoing deck's CFX knob to high-pass while bringing the other in, then centre it. Verify there are no clicks or zipper noise on fast sweeps or across the centre, the centred filter sounds identical to before, the headphones hear the filter, the screen knob follows the hardware (and takes over correctly after an on-screen reset), and the overrun counter stays at 0

## Workflow follow-up

- Archive `add-v0-1-walking-skeleton` before archiving this change, so `controller-input` has a main spec to add to.
