//! djkitty: the DJ app that grows with you.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod app;
mod audio;
mod columns;
mod devtools;
mod layout_dump;
mod loader;
mod settings;
mod sheen;
mod shortcuts;
mod theme;
mod ui;
mod util;
mod vault_ui;
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

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("DJKitty")
        .with_inner_size([1280.0, 760.0])
        .with_min_inner_size([960.0, 600.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    // The app id names the settings folder, so it stays lowercase.
    eframe::run_native(
        "djkitty",
        options,
        Box::new(|cc| Ok(Box::new(app::DjApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

/// The logo, rasterised for the window and taskbar icon.
fn window_icon() -> Option<egui::IconData> {
    const SIZE: u32 = 256;
    let tree = resvg::usvg::Tree::from_data(
        include_bytes!("../assets/logo.svg"),
        &resvg::usvg::Options::default(),
    )
    .ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE)?;
    let scale = SIZE as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied alpha; the window icon wants straight alpha.
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Some(egui::IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_icon_renders_the_logo() {
        let icon = super::window_icon().expect("logo.svg rasterises");
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        // The centre of the tile is the cat's (opaque, light) face.
        let i = (128 * 256 + 128) * 4;
        assert_eq!(icon.rgba[i + 3], 255);
        assert!(icon.rgba[i] > 200, "{:?}", &icon.rgba[i..i + 4]);
    }
}
