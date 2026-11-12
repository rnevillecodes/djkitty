# track-analysis Specification

## Purpose
Gives every track the stats a DJ needs to plan and mix (file properties, tempo, musical key) and the waveform data the UI draws, computed once and reused.

## Requirements

### Requirement: File stats
The system SHALL record each track's duration, container/codec format, sample rate, channel count and bitrate (or "lossless" for lossless formats), and SHALL show them in a track details view.

#### Scenario: Viewing file stats
- **WHEN** the user opens the details of an imported 320 kbps MP3 at 44.1 kHz
- **THEN** the details show format MP3, 44100 Hz, 2 channels, 320 kbps and the duration in mm:ss

### Requirement: BPM detection
The system SHALL estimate a tempo for each track in the range 60–200 BPM, shown to one decimal place. For steady-tempo electronic music the estimate SHALL be within ±1 BPM of the true tempo or exactly half or double it.

#### Scenario: Steady four-on-the-floor track
- **WHEN** a track with a constant 128 BPM kick drum is analysed
- **THEN** its BPM is shown as a value between 127.0 and 129.0, or between 63.5 and 64.5 if detected at half-time

#### Scenario: No detectable beat
- **WHEN** a track without a clear pulse (e.g. ambient) is analysed
- **THEN** its BPM is shown as unknown rather than a guessed value

### Requirement: Key detection
The system SHALL estimate each track's musical key on a best-effort basis and SHALL display it in both Camelot notation (e.g. 8A) and standard notation (e.g. A minor). It SHALL show unknown when no estimate is confident enough.

#### Scenario: Key shown in both notations
- **WHEN** a track is analysed and estimated to be in A minor
- **THEN** its key is shown as `8A` with `A minor` visible in the details view

### Requirement: Loudness measurement
The system SHALL measure each track's integrated loudness as defined in ITU-R BS.1770-4 / EBU R128 (K-weighted, gated) and SHALL show it in LUFS to one decimal place. It SHALL show unknown for tracks that are silent or shorter than 400 ms.

#### Scenario: Reference tone
- **WHEN** a track holding a full-scale 997 Hz sine on both channels is analysed
- **THEN** its loudness is shown as a value between −0.1 and 0.1 LUFS

#### Scenario: Quiet intro does not lower the reading
- **WHEN** a track starts with silence and a breakdown 30 dB quieter than the rest
- **THEN** its loudness is within 0.2 LU of the loud part measured on its own

### Requirement: Background analysis
The system SHALL analyse newly imported tracks in the background without blocking the UI or audio playback, SHALL show progress, and SHALL show results as each track finishes.

#### Scenario: Playing while analysing
- **WHEN** 200 tracks are being analysed and the user plays a track on a deck
- **THEN** playback has no audible dropouts and the UI stays responsive

#### Scenario: Progress
- **WHEN** analysis is running
- **THEN** the UI shows how many tracks are done out of the total

### Requirement: Analysis is cached
The system SHALL store analysis results and waveform data so a track is analysed only once, and SHALL re-analyse it only if its file has changed (different size or modification time).

#### Scenario: Restart does not re-analyse
- **WHEN** the app restarts after a library has been fully analysed
- **THEN** no tracks are re-analysed

#### Scenario: File replaced
- **WHEN** a track's file is overwritten with a different file at the same path
- **THEN** that track is analysed again

### Requirement: Waveform data
The system SHALL produce waveform data for each track that gives amplitude over time at enough resolution to draw both a full-track overview and a zoomed-in scrolling view, with a separate level for low, mid and high frequencies.

#### Scenario: Waveform available after analysis
- **WHEN** a track's analysis completes
- **THEN** its overview waveform can be drawn without decoding the audio file again
