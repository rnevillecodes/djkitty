//! Vaults: the folder that holds a whole library.
//!
//! ```text
//! <root>\
//!   .djkitty\vault.toml      marker (format = 1)
//!   .djkitty\library.db
//!   .djkitty\waveforms\      waveform cache
//!   .djkitty\incoming\       copies in progress (*.part)
//!   Catalog\<artist>\<file>  the audio
//!   Sets\<set>.m3u8          playlist mirror of each set
//! ```
//!
//! Every path the library stores is relative to the root, so a vault can be moved,
//! renamed or opened on another computer.

use std::path::{Path, PathBuf};

use crate::{LibraryError, Result, names};

/// Vault format this build writes and understands.
pub const VAULT_FORMAT: u32 = 1;

const META_DIR: &str = ".djkitty";
const MARKER: &str = "vault.toml";
pub(crate) const CATALOG_DIR: &str = "Catalog";
pub(crate) const SETS_DIR: &str = "Sets";

/// An open vault folder (its layout; the library inside is opened with
/// [`crate::Library::open`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    /// Create the vault `<parent>\<name>` and its folders. The name is trimmed and must be
    /// usable as a folder name as typed; the folder must not exist yet.
    pub fn create(parent: &Path, name: &str) -> Result<Vault> {
        let name = name.trim();
        if name.is_empty() || names::sanitize_component(name) != name {
            return Err(LibraryError::InvalidName(name.to_owned()));
        }
        if !parent.is_dir() {
            return Err(LibraryError::Unavailable(parent.to_path_buf()));
        }
        let root = parent.join(name);
        if root.exists() {
            return Err(LibraryError::AlreadyExists(root));
        }
        // `create_dir` (not `_all`) so a folder that appeared in the meantime is an error.
        std::fs::create_dir(&root).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => LibraryError::AlreadyExists(root.clone()),
            _ => LibraryError::Io(e),
        })?;
        let vault = Vault { root };
        vault.create_layout()?;
        // The marker goes last: a half-created folder is never mistaken for a vault.
        std::fs::write(
            vault.meta_dir().join(MARKER),
            format!("# djkitty vault\nformat = {VAULT_FORMAT}\n"),
        )?;
        Ok(vault)
    }

    /// Open the vault at `root`. Fails without creating anything if the folder is not
    /// there, is not a vault, or was made by a newer djkitty.
    pub fn open(root: &Path) -> Result<Vault> {
        if !root.is_dir() {
            return Err(LibraryError::Unavailable(root.to_path_buf()));
        }
        let marker = root.join(META_DIR).join(MARKER);
        let text = match std::fs::read_to_string(&marker) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(LibraryError::NotAVault(root.to_path_buf()));
            }
            Err(e) => return Err(e.into()),
        };
        let format = text
            .lines()
            .filter_map(|l| l.split_once('='))
            .find(|(k, _)| k.trim() == "format")
            .and_then(|(_, v)| v.trim().parse::<u32>().ok())
            .ok_or_else(|| LibraryError::NotAVault(root.to_path_buf()))?;
        if format > VAULT_FORMAT {
            return Err(LibraryError::NewerFormat(root.to_path_buf()));
        }
        let vault = Vault {
            root: root.to_path_buf(),
        };
        // A vault copied without its empty folders still works.
        vault.create_layout()?;
        Ok(vault)
    }

    fn create_layout(&self) -> Result<()> {
        for dir in [
            self.meta_dir(),
            self.waveforms_dir(),
            self.incoming_dir(),
            self.catalog_dir(),
            self.sets_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Whether `root` holds a vault marker (without opening it).
    pub fn is_vault(root: &Path) -> bool {
        root.join(META_DIR).join(MARKER).is_file()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The vault's name: its folder name.
    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string())
    }

    pub fn meta_dir(&self) -> PathBuf {
        self.root.join(META_DIR)
    }

    pub fn db_path(&self) -> PathBuf {
        self.meta_dir().join("library.db")
    }

    pub fn waveforms_dir(&self) -> PathBuf {
        self.meta_dir().join("waveforms")
    }

    pub fn incoming_dir(&self) -> PathBuf {
        self.meta_dir().join("incoming")
    }

    pub fn catalog_dir(&self) -> PathBuf {
        self.root.join(CATALOG_DIR)
    }

    pub fn sets_dir(&self) -> PathBuf {
        self.root.join(SETS_DIR)
    }

    /// Absolute path of a vault-relative path stored with `/` separators.
    pub fn resolve(&self, rel: &str) -> PathBuf {
        let mut p = self.root.clone();
        for part in rel.split('/') {
            p.push(part);
        }
        p
    }

    /// Delete leftover in-progress copies (from an import or relink that never finished).
    pub(crate) fn clear_incoming(&self) {
        let Ok(entries) = std::fs::read_dir(self.incoming_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Err(e) = std::fs::remove_file(&path)
            {
                tracing::warn!("cannot remove leftover {}: {e}", path.display());
            }
        }
    }
}

/// A vault-relative path stored by the library points into the Catalog and nowhere else:
/// relative, `/`-separated, starting with `Catalog/`, no `.`/`..` or empty parts.
pub(crate) fn is_catalog_rel(rel: &str) -> bool {
    let mut parts = rel.split('/');
    parts.next() == Some(CATALOG_DIR)
        && rel
            .split('/')
            .skip(1)
            .all(|p| !p.is_empty() && p != "." && p != ".." && !p.contains(['\\', ':']))
        && rel.split('/').count() >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_makes_the_layout_and_opens() {
        let tmp = tempfile::tempdir().unwrap();
        let v = Vault::create(tmp.path(), "My DJ Vault").unwrap();
        let root = tmp.path().join("My DJ Vault");
        assert_eq!(v.root(), root);
        for d in [".djkitty", "Catalog", "Sets", ".djkitty/waveforms"] {
            assert!(root.join(d).is_dir(), "{d}");
        }
        assert!(Vault::is_vault(&root));
        assert_eq!(Vault::open(&root).unwrap(), v);
        assert_eq!(v.name(), "My DJ Vault");
    }

    #[test]
    fn name_is_trimmed() {
        let tmp = tempfile::tempdir().unwrap();
        let v = Vault::create(tmp.path(), "  Gig Vault ").unwrap();
        assert_eq!(v.root(), tmp.path().join("Gig Vault"));
    }

    #[test]
    fn invalid_names_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["", "   ", "a:b", "what?", "CON", "dots...", "a/b"] {
            assert!(
                matches!(
                    Vault::create(tmp.path(), name),
                    Err(LibraryError::InvalidName(_))
                ),
                "{name:?}"
            );
        }
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn existing_folder_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("Taken")).unwrap();
        assert!(matches!(
            Vault::create(tmp.path(), "Taken"),
            Err(LibraryError::AlreadyExists(_))
        ));
        assert_eq!(
            std::fs::read_dir(tmp.path().join("Taken")).unwrap().count(),
            0
        );
    }

    #[test]
    fn opening_a_plain_folder_is_refused_and_changes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let music = tmp.path().join("Music");
        std::fs::create_dir(&music).unwrap();
        std::fs::write(music.join("a.mp3"), b"x").unwrap();
        assert!(matches!(
            Vault::open(&music),
            Err(LibraryError::NotAVault(_))
        ));
        let names: Vec<_> = std::fs::read_dir(&music)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("a.mp3")]);
    }

    #[test]
    fn missing_folder_is_unavailable() {
        let tmp = tempfile::tempdir().unwrap();
        let gone = tmp.path().join("Gig Vault");
        assert!(matches!(
            Vault::open(&gone),
            Err(LibraryError::Unavailable(_))
        ));
        assert!(!gone.exists());
    }

    #[test]
    fn newer_format_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let v = Vault::create(tmp.path(), "V").unwrap();
        std::fs::write(v.meta_dir().join(MARKER), "format = 99\n").unwrap();
        assert!(matches!(
            Vault::open(v.root()),
            Err(LibraryError::NewerFormat(_))
        ));
    }

    #[test]
    fn catalog_rel_paths() {
        assert!(is_catalog_rel("Catalog/Bicep/02 Glue.mp3"));
        assert!(!is_catalog_rel("Catalog"));
        assert!(!is_catalog_rel("Catalog/../outside.mp3"));
        assert!(!is_catalog_rel("Sets/x.m3u8"));
        assert!(!is_catalog_rel("C:/x.mp3"));
        assert!(!is_catalog_rel("Catalog/C:\\x.mp3"));
        assert!(!is_catalog_rel("Catalog//x.mp3"));
    }
}
