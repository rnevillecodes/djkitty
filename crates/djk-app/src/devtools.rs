//! Developer command-line tools for hardware discovery.

use anyhow::Context;

/// `--list-audio-devices`: print every output device and what it supports.
pub fn list_audio_devices() -> anyhow::Result<()> {
    let devices = djk_audio::device::list_output_devices().context("listing audio devices")?;
    if devices.is_empty() {
        println!("No audio output devices found.");
    }
    for d in devices {
        let default = if d.is_default {
            " (system default)"
        } else {
            ""
        };
        println!("{}{default}", d.name);
        if let Some((ch, sr)) = d.default_config {
            println!("  default: {ch} ch @ {sr} Hz");
        }
        println!("  max output channels: {}", d.max_channels());
        // The engine renders f32, so only list f32 configurations, one line per
        // channel count / rate range.
        let mut seen = Vec::new();
        for c in d.configs.iter().filter(|c| c.sample_format == "F32") {
            let key = (
                c.channels,
                c.min_sample_rate,
                c.max_sample_rate,
                c.buffer_frames,
            );
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let rate = if c.min_sample_rate == c.max_sample_rate {
                format!("{} Hz", c.min_sample_rate)
            } else {
                format!("{}..{} Hz", c.min_sample_rate, c.max_sample_rate)
            };
            let buf = match c.buffer_frames {
                Some((min, max)) if min == max => format!("buffer {min} frames"),
                Some((min, max)) => format!("buffer {min}..{max} frames"),
                None => "buffer size unknown".to_owned(),
            };
            println!("  - {} ch, {rate}, {buf}", c.channels);
        }
    }
    Ok(())
}

/// `--midi-monitor [filter]`: log raw MIDI from matching ports and send typed hex.
/// Pass an empty string (`--midi-monitor ""`) to open every port.
pub fn midi_monitor(filter: &str) -> anyhow::Result<()> {
    djk_controller::monitor::run(filter).map_err(|e| anyhow::anyhow!("{e}"))
}
