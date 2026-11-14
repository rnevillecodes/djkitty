//! SQLite-backed track library living inside a vault: import (copying into the vault's
//! Catalog), sets (mirrored as playlist files) and analysis results.
//!
//! The database, the waveform cache and every audio file live inside a [`Vault`] folder.
//! Paths are stored relative to the vault, and the library hands out absolute paths.

mod files;
mod import;
pub mod names;
mod schema;
mod sets;
mod tracks;
mod vault;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

pub use files::FileRemoval;
pub use import::{ImportSummary, read_tags};
pub use schema::LATEST as SCHEMA_VERSION;
pub use vault::{VAULT_FORMAT, Vault};

/// Version of the analysis results the library stores. Bump it whenever analysis changes
/// in a way that alters results, so existing tracks are re-analysed in the background.
///
/// 2: BPM detection scores whole beat combs over a 90–180 BPM range (fixes drum & bass
///    at 174 BPM reading as 116).
/// 3: beatgrid detection (phase, downbeat and fine-tuned tempo).
/// 4: beatgrids start at the first audible beat (bar 1 beat 1).
/// 5: integrated loudness (LUFS).
pub const ANALYSIS_VERSION: i64 = 5;
pub use sets::{SetId, SetSummary};
pub use tracks::{
    AnalysisRecord, SortKey, StoredGrid, TrackQuery, TrackRow, matches_search as tracks_match,
    sort_tracks,
};

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("track not found")]
    NotFound,
    #[error("file is missing: {0}")]
    Missing(PathBuf),
    #[error("cannot find a data directory for this platform")]
    NoDataDir,
    #[error("“{0}” can't be used as a folder name")]
    InvalidName(String),
    #[error("{0} already exists")]
    AlreadyExists(PathBuf),
    #[error("{0} is not available")]
    Unavailable(PathBuf),
    #[error("{0} is not a djkitty vault")]
    NotAVault(PathBuf),
    #[error("{0} was made by a newer version of djkitty")]
    NewerFormat(PathBuf),
    #[error("that file is not the same as the missing one")]
    NotSameFile,
    #[error("a different file is already at {0}")]
    Occupied(PathBuf),
    #[error("could not copy into the vault: {0}")]
    VaultWrite(std::io::Error),
}

pub type Result<T> = std::result::Result<T, LibraryError>;

/// The track library of one vault.
pub struct Library {
    conn: Connection,
    vault: Vault,
    /// Problems that did not fail an operation (e.g. a set's playlist file could not be
    /// written), for the app to show. See [`Library::take_warnings`].
    warnings: Vec<String>,
}

/// Per-computer data directory for djkitty (settings), e.g. `%APPDATA%\djkitty\data`
/// on Windows. The `DJKITTY_DATA_DIR` environment variable overrides it (useful for
/// testing).
pub fn default_data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("DJKITTY_DATA_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "djkitty")
        .map(|p| p.data_dir().to_path_buf())
        .ok_or(LibraryError::NoDataDir)
}

impl Library {
    /// Open (creating if needed) the library of a vault. Leftover partial copies from an
    /// interrupted import are removed and every set's playlist file is rewritten.
    pub fn open(vault: &Vault) -> Result<Self> {
        vault.clear_incoming();
        let conn = Connection::open(vault.db_path())?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        schema::migrate(&conn)?;
        let mut lib = Self {
            conn,
            vault: vault.clone(),
            warnings: Vec::new(),
        };
        lib.write_all_set_files()?;
        Ok(lib)
    }

    /// Open the vault at `root` and its library.
    pub fn open_at(root: &Path) -> Result<Self> {
        Self::open(&Vault::open(root)?)
    }

    /// A second connection to the same vault for a worker thread: no cleanup or set file
    /// rewriting (the owner already did that, and must not race with it).
    pub fn open_worker(vault: &Vault) -> Result<Self> {
        let conn = Connection::open(vault.db_path())?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        schema::migrate(&conn)?;
        Ok(Self {
            conn,
            vault: vault.clone(),
            warnings: Vec::new(),
        })
    }

    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    /// Path of a track's waveform cache file.
    pub fn waveform_path(&self, id: djk_core::TrackId) -> PathBuf {
        self.vault.waveforms_dir().join(format!("{}.wf", id.0))
    }

    /// Schema version (for tests and diagnostics).
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// Warnings gathered since the last call (each is a sentence for the user).
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    pub(crate) fn warn(&mut self, msg: String) {
        tracing::warn!("{msg}");
        self.warnings.push(msg);
    }
}

/// File size and modification time (seconds since the Unix epoch), used to detect changes.
pub(crate) fn file_stamp(path: &Path) -> std::io::Result<(i64, i64)> {
    let meta = std::fs::metadata(path)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    Ok((meta.len() as i64, mtime))
}
