//! djkitty: the DJ app that grows with you.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod app;
mod audio;
mod columns;
mod devtools;
mod loader;
mod settings;
mod shortcuts;
mod theme;
mod ui;
mod util;
mod waveview;
mod widgets;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list-audio-devices") {
        return devtools::list_audio_devices();
    }
    if let Some(i) = args.iter().position(|a| a == "--test-tone") {
        let route = args.get(i + 1).map(String::as_str).unwrap_or("master");
        return devtools::test_tone(route);
    }
    if let Some(i) = args.iter().position(|a| a == "--import-folder") {
        let folder = args
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("--import-folder needs a folder path"))?;
        return devtools::import_folder(std::path::Path::new(folder));
    }
    if let Some(i) = args.iter().position(|a| a == "--analyse") {
        let paths: Vec<std::path::PathBuf> = args[i + 1..]
            .iter()
            .take_while(|a| !a.starts_with("--"))
            .map(std::path::PathBuf::from)
            .collect();
        return devtools::analyse(&paths);
    }
    if let Some(i) = args.iter().position(|a| a == "--midi-monitor") {
        let filter = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned();
        return devtools::midi_monitor(filter.as_deref().unwrap_or("DDJ-FLX2"));
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "info,wgpu=error,wgpu_hal=error,wgpu_core=error,naga=warn".into()
            }),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("djkitty")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "djkitty",
        options,
        Box::new(|cc| Ok(Box::new(app::DjApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
