# Proposal

## Why

The walking skeleton, hot cues and loops, and channel filter changes are fully built, but each still had unticked tasks whose only remaining work was a check: recording what the DDJ-FLX2 actually sends, trying features on the hardware, listening, or a long by-hand pass through the UI. Those checks kept all three changes (and their specs) from being archived, and they are best done together in one session with the controller anyway.

## What Changes

- Collects every outstanding hardware, listening and by-hand check from `add-v0-1-walking-skeleton`, `add-hot-cues-and-loops` and `add-channel-filter` into one checklist. Each task names the original task it came from.
- Covers hardware discovery (the FLX2's audio interface, every control's MIDI messages, LED messages, pad modes, the CFX knobs), applying the recorded numbers to the mapping (replacing the provisional DDJ-FLX2/FLX4 numbers, with unit tests on the recorded bytes), hardware checks, listening checks, the remaining no-controller UI checks, and the end-to-end mixing sessions.
- No new behaviour. If a check fails, the fix is made here (or in a follow-up change) against the archived specs.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

_None._ This change verifies the requirements already specified in `audio-mixing`, `deck-playback`, `controller-input`, `track-library`, `set-management`, `track-analysis`, `waveform-display`, `beatgrid`, `hot-cues`, `loops` and `channel-filter`. If a recording shows the hardware behaves differently from a spec (for example, the FLX2 owns its pad mode), the spec change is made here as a delta.

## Impact

- `djk-controller`: the DDJ-FLX2 mapping table, LED messages and their unit tests, once the real messages are recorded.
- `docs/controllers/ddj-flx2.md`: the observed columns and the final control → action table; the README stops calling the mapping provisional.
- No library, file-format or engine changes are expected.
