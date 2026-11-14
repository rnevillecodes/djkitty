//! Per-computer settings: a tiny `key=value` file in the data directory. Nothing here
//! belongs to a vault; vaults carry their own library.

use std::path::{Path, PathBuf};

/// How many recently opened vaults are remembered.
pub const RECENT_VAULTS: usize = 5;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Settings {
    /// Name of the audio output device the user picked, if any.
    pub audio_device: Option<String>,
    /// Track list column widths, as last resized.
    pub track_columns: Option<Vec<f32>>,
    /// The vault open when djkitty last ran, reopened on launch.
    pub last_vault: Option<PathBuf>,
    /// Recently opened vaults, most recent first (includes `last_vault`).
    pub recent_vaults: Vec<PathBuf>,
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
                        "last_vault" if !v.is_empty() => s.last_vault = Some(PathBuf::from(v)),
                        "recent_vaults" => {
                            s.recent_vaults = v
                                .split('|')
                                .map(str::trim)
                                .filter(|p| !p.is_empty())
                                .map(PathBuf::from)
                                .take(RECENT_VAULTS)
                                .collect();
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
        if let Some(v) = &self.last_vault {
            text.push_str(&format!("last_vault={}\n", v.display()));
        }
        if !self.recent_vaults.is_empty() {
            let recent: Vec<String> = self
                .recent_vaults
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            text.push_str(&format!("recent_vaults={}\n", recent.join("|")));
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(path, text) {
            tracing::warn!("cannot save settings: {e}");
        }
    }

    /// Record `root` as the vault just opened: the last vault, and first in the recent
    /// list (without duplicates, keeping at most [`RECENT_VAULTS`]).
    pub fn remember_vault(&mut self, root: &Path) {
        self.last_vault = Some(root.to_path_buf());
        self.recent_vaults.retain(|p| !same_path(p, root));
        self.recent_vaults.insert(0, root.to_path_buf());
        self.recent_vaults.truncate(RECENT_VAULTS);
    }
}

/// Paths compared the way Windows does: ignoring letter case and separator style.
fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    norm(a) == norm(b)
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
        assert_eq!(s.last_vault, None);
        assert!(s.recent_vaults.is_empty());
        s.audio_device = Some("Speakers (DDJ-FLX2)".into());
        s.track_columns = Some(vec![300.0, 180.5, 60.0]);
        s.remember_vault(Path::new(r"E:\Gig Vault"));
        s.remember_vault(Path::new(r"D:\My DJ Vault"));
        s.save();
        let loaded = Settings::load(dir.path());
        assert_eq!(loaded.audio_device.as_deref(), Some("Speakers (DDJ-FLX2)"));
        assert_eq!(loaded.track_columns, Some(vec![300.0, 180.5, 60.0]));
        assert_eq!(loaded.last_vault, Some(PathBuf::from(r"D:\My DJ Vault")));
        assert_eq!(
            loaded.recent_vaults,
            vec![
                PathBuf::from(r"D:\My DJ Vault"),
                PathBuf::from(r"E:\Gig Vault")
            ]
        );
    }

    #[test]
    fn recent_vaults_are_deduplicated_and_capped() {
        let mut s = Settings::default();
        for i in 0..7 {
            s.remember_vault(&PathBuf::from(format!(r"D:\V{i}")));
        }
        s.remember_vault(Path::new(r"d:/v3"));
        assert_eq!(s.recent_vaults.len(), RECENT_VAULTS);
        assert_eq!(s.recent_vaults[0], PathBuf::from("d:/v3"));
        assert_eq!(
            s.recent_vaults
                .iter()
                .filter(|p| same_path(p, Path::new(r"D:\V3")))
                .count(),
            1
        );
        assert_eq!(s.last_vault, Some(PathBuf::from("d:/v3")));
    }
}
