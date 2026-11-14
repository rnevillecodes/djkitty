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

/// `--test-tone [master|headphones|silent]`: play a 440 Hz tone for 3 seconds through the
/// selected device's master or headphone output, then report stream statistics.
/// `silent` runs the stream at zero level, to check the device opens without sound.
pub fn test_tone(route: &str) -> anyhow::Result<()> {
    use djk_core::{Command, DeckId, LoadRequest, TrackBuffer};
    use std::sync::{Arc, Mutex};

    let (engine, mut handles) = djk_audio::Engine::new(48_000, 2);
    let engine = Arc::new(Mutex::new(engine));
    let output = djk_audio::output::open(engine.clone(), None)?;
    println!(
        "Opened {} — {} ch @ {} Hz, buffer {:?} frames",
        output.device_name, output.channels, output.sample_rate, output.buffer_frames
    );

    let rate = 48_000;
    let samples: Vec<f32> = (0..rate * 3)
        .flat_map(|i| {
            let v = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin() * 0.25;
            [v, v]
        })
        .collect();
    let buffer = TrackBuffer {
        samples: Arc::from(samples),
        sample_rate: rate as u32,
    };
    handles
        .load(LoadRequest {
            deck: DeckId::A,
            buffer,
            token: 1,
            hot_cues: Default::default(),
        })
        .map_err(|_| anyhow::anyhow!("load queue full"))?;
    match route {
        "headphones" => {
            if output.channels < 4 {
                println!(
                    "Device has {} channels: headphone output unavailable.",
                    output.channels
                );
            }
            handles.send(Command::SetChannelFader {
                deck: DeckId::A,
                value: 0.0,
            });
            handles.send(Command::SetHeadphoneCue {
                deck: DeckId::A,
                enabled: true,
            });
            handles.send(Command::SetHeadphoneLevel(1.0));
        }
        "silent" => {
            handles.send(Command::SetMasterLevel(0.0));
            handles.send(Command::SetHeadphoneLevel(0.0));
        }
        _ => {}
    }
    handles.send(Command::Play(DeckId::A));
    for _ in 0..35 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        handles.drop_retired();
        if output.is_disconnected() {
            println!("Device disconnected during playback (no crash).");
            break;
        }
    }
    let s = *handles.state.read();
    println!(
        "Rendered {} frames, output latency {:.1} ms, overruns {}",
        s.frames_rendered,
        s.output_latency * 1000.0,
        s.overruns
    );
    Ok(())
}

/// `--analyse [paths…]`: print BPM, key, loudness and beatgrid for files (folders are searched). With no
/// paths, re-analyses every track in the last opened vault and shows the stored BPM
/// alongside. Audio files are only read and no results are saved.
pub fn analyse(paths: &[std::path::PathBuf]) -> anyhow::Result<()> {
    // (file, stored BPM if known)
    let mut files: Vec<(std::path::PathBuf, Option<f32>)> = Vec::new();
    if paths.is_empty() {
        let lib = last_vault_library()?;
        for t in lib.tracks(&djk_library::TrackQuery::default())? {
            files.push((t.path.clone(), t.bpm));
        }
        println!(
            "Analysing {} library tracks (nothing is saved)\n",
            files.len()
        );
    } else {
        fn walk(p: &std::path::Path, out: &mut Vec<(std::path::PathBuf, Option<f32>)>) {
            if p.is_dir() {
                if let Ok(rd) = std::fs::read_dir(p) {
                    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
                    entries.sort();
                    for e in entries {
                        walk(&e, out);
                    }
                }
            } else if djk_analysis::is_supported(p) {
                out.push((p.to_path_buf(), None));
            }
        }
        for p in paths {
            walk(p, &mut files);
        }
    }
    println!(
        "{:>8}  {:>8}  {:>4}  {:>6}  {:>18}  file",
        "stored", "now", "key", "LUFS", "grid"
    );
    for (path, stored) in files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match djk_analysis::analyse_file(&path) {
            Ok(a) => println!(
                "{:>8}  {:>8}  {:>4}  {:>6}  {:>18}  {name}",
                stored
                    .map(|b| format!("{b:.1}"))
                    .unwrap_or_else(|| "-".into()),
                a.bpm
                    .map(|b| format!("{b:.2}"))
                    .unwrap_or_else(|| "?".into()),
                a.key.map(|k| k.camelot()).unwrap_or_else(|| "?".into()),
                a.lufs
                    .map(|l| format!("{l:.1}"))
                    .unwrap_or_else(|| "?".into()),
                a.grid
                    .map(|g| format!("starts at {:.3} s", g.start_secs))
                    .unwrap_or_else(|| "no grid".into()),
            ),
            Err(e) => println!("{:>8}  {:>8}  {:>4}  {name}  ({e})", "-", "error", ""),
        }
    }
    Ok(())
}

/// `--import-folder <path>` and `--analyse` work on the vault djkitty last had open on
/// this computer (settings honour `DJKITTY_DATA_DIR`).
/// The library of the vault djkitty last had open on this computer.
fn last_vault_library() -> anyhow::Result<djk_library::Library> {
    let dir = djk_library::default_data_dir()?;
    let settings = crate::settings::Settings::load(&dir);
    let Some(root) = settings.last_vault else {
        anyhow::bail!("no vault yet: open a vault in djkitty first");
    };
    djk_library::Library::open_at(&root)
        .map_err(|e| anyhow::anyhow!("cannot open the last vault: {e}"))
}

/// `--import-folder <path>`: import a folder into the last opened vault without the UI,
/// then analyse everything that needs it.
pub fn import_folder(folder: &std::path::Path) -> anyhow::Result<()> {
    let mut lib = last_vault_library()?;
    let s = lib.import_folder(folder, |_| {})?;
    println!(
        "Imported into {}: {} added ({:.1} MB copied), {} already in the vault, {} skipped, \
         {} ignored",
        lib.vault().root().display(),
        s.added,
        s.bytes_copied as f64 / 1e6,
        s.existing,
        s.skipped,
        s.ignored
    );
    let vault = lib.vault().clone();
    let pool = crate::analysis::AnalysisPool::start(&vault, crate::analysis::worker_count(), || {});
    let queued = pool.rescan(&mut lib);
    while pool.is_busy() {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    println!("Analysed {queued} track(s)");
    Ok(())
}
