//! Persisted user settings: a tiny `key=value` file in the data directory.

use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Settings {
    /// Name of the audio output device the user picked, if any.
    pub audio_device: Option<String>,
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
                    match k.trim() {
                        "audio_device" if !v.trim().is_empty() => {
                            s.audio_device = Some(v.trim().to_owned());
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
        let text = format!(
            "audio_device={}\n",
            self.audio_device.as_deref().unwrap_or("")
        );
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
        s.audio_device = Some("Speakers (DDJ-FLX2)".into());
        s.save();
        assert_eq!(
            Settings::load(dir.path()).audio_device.as_deref(),
            Some("Speakers (DDJ-FLX2)")
        );
    }
}
