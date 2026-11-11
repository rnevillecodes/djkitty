//! Persisted user settings: a tiny `key=value` file in the data directory.

use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Settings {
    /// Name of the audio output device the user picked, if any.
    pub audio_device: Option<String>,
    /// Track list column widths, as last resized.
    pub track_columns: Option<Vec<f32>>,
    path: Option<PathBuf>,
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.txt");
        let mut s = Settings {
            path: Some(path.clone()),
            ..Default::default()
        };
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    let v = v.trim();
                    match k.trim() {
                        "audio_device" if !v.is_empty() => {
                            s.audio_device = Some(v.to_owned());
                        }
                        "track_columns" => {
                            s.track_columns = v.split(',').map(|w| w.trim().parse().ok()).collect();
                        }
                        _ => {}
                    }
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let mut text = format!(
            "audio_device={}\n",
            self.audio_device.as_deref().unwrap_or("")
        );
        if let Some(cols) = &self.track_columns {
            let cols: Vec<String> = cols.iter().map(|w| format!("{w:.1}")).collect();
            text.push_str(&format!("track_columns={}\n", cols.join(",")));
        }
        if let Err(e) = std::fs::write(path, text) {
            tracing::warn!("cannot save settings: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Settings::load(dir.path());
        assert_eq!(s.audio_device, None);
        assert_eq!(s.track_columns, None);
        s.audio_device = Some("Speakers (DDJ-FLX2)".into());
        s.track_columns = Some(vec![300.0, 180.5, 60.0]);
        s.save();
        let loaded = Settings::load(dir.path());
        assert_eq!(loaded.audio_device.as_deref(), Some("Speakers (DDJ-FLX2)"));
        assert_eq!(loaded.track_columns, Some(vec![300.0, 180.5, 60.0]));
    }
}
