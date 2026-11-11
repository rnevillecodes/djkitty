# Pioneer DJ / AlphaTheta DDJ-FLX2

Status: **mapping provisional, hardware verification pending.**

The message numbers in `crates/djk-controller/src/ddj_flx2.rs` currently follow the
Pioneer DDJ-FLX4 conventions (the FLX2's larger sibling). Each one must be confirmed on a
real FLX2 with the MIDI monitor and this file updated with what was observed.

## How to discover messages

```bash
cargo run -p djk-app -- --list-audio-devices
cargo run -p djk-app -- --midi-monitor
```

`--midi-monitor` opens every MIDI input whose name contains `DDJ-FLX2` (pass another filter,
or `""` for all ports) and prints each message as hex plus a decoded form. Type hex bytes
such as `90 0B 7F` and press Enter to send them to the controller (used for LED discovery).

## Audio interface (task 1.3)

| Question                          | Observed |
|-----------------------------------|----------|
| Device name in Windows            | _todo_   |
| Output channels exposed           | _todo_ (4 expected: master 1-2, headphones 3-4) |
| Sample rates                      | _todo_   |
| WASAPI buffer size range          | _todo_   |

## Incoming messages (task 1.4)

Status bytes: deck 1 = channel 1 (`0x90`/`0xB0`), deck 2 = channel 2 (`0x91`/`0xB1`),
mixer/global = channel 7 (`0x96`/`0xB6`) on the FLX4. 14-bit faders send an MSB CC and an
LSB CC (MSB number + 0x20).

| Control               | Expected (FLX4)                     | Observed on FLX2 |
|-----------------------|-------------------------------------|------------------|
| Play/Pause            | note `0x0B`, deck channel           | _todo_ |
| Cue                   | note `0x0C`, deck channel           | _todo_ |
| Tempo (pitch) fader   | CC `0x00` MSB / `0x20` LSB          | _todo_ |
| Jog wheel, side       | CC `0x21`, relative, 64 = still     | _todo_ |
| Jog wheel, top        | CC `0x22`, relative, 64 = still     | _todo_ |
| Jog touch             | note `0x36`                         | _todo_ |
| Channel fader         | CC `0x13` MSB / `0x33` LSB          | _todo_ |
| EQ HI knob            | CC `0x07` MSB / `0x27` LSB          | _todo_ |
| EQ MID knob           | CC `0x0B` MSB / `0x2B` LSB          | _todo_ |
| EQ LOW knob           | CC `0x0F` MSB / `0x2F` LSB          | _todo_ |
| Headphone cue         | note `0x54`, deck channel           | _todo_ |
| Crossfader            | CC `0x1F` MSB / `0x3F` LSB, ch 7    | _todo_ |
| Browse encoder        | CC `0x40`, ch 7, relative           | _todo_ |
| Load deck 1 / deck 2  | note `0x46` / `0x47`, ch 7          | _todo_ |

## LED messages (task 1.5)

Expected: send the same note number back on the same channel, velocity `0x7F` = on and
`0x00` = off.

| LED               | Expected message      | Observed |
|-------------------|-----------------------|----------|
| Play deck 1       | `90 0B 7F` / `90 0B 00` | _todo_ |
| Cue deck 1        | `90 0C 7F` / `90 0C 00` | _todo_ |
| Headphone cue 1   | `90 54 7F` / `90 54 00` | _todo_ |
| (deck 2: status `91`) |                   |          |

## Initial control positions

Faders and knobs only send MIDI when moved, so on connect the app sends the DDJ-400/FLX4
"report current positions" SysEx (`F0 00 40 05 00 00 02 06 00 03 01 F7`, as Mixxx does)
and applies whatever comes back. Until something is changed on screen the hardware is
the truth, so the first message from each fader is applied without waiting for pickup.

| Question                                   | Observed |
|--------------------------------------------|----------|
| FLX2 answers the SysEx with all positions  | _todo_   |
