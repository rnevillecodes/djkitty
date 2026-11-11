//! SQLite-backed track library, folder import and sets.
//!
//! The library stores tracks (identified by path), their analysis results and user sets.
//! Waveform data lives in per-track cache files next to the database (see
//! [`Library::waveform_path`]).

mod import;
mod schema;
mod sets;
mod tracks;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

pub use import::{ImportSummary, read_tags};
pub use schema::LATEST as SCHEMA_VERSION;

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
}

pub type Result<T> = std::result::Result<T, LibraryError>;

/// The track library.
pub struct Library {
    conn: Connection,
    cache_dir: PathBuf,
}

/// Platform data directory for djkitty, e.g. `%APPDATA%\djkitty\data` on Windows.
/// The `DJKITTY_DATA_DIR` environment variable overrides it (useful for testing).
pub fn default_data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("DJKITTY_DATA_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "djkitty")
        .map(|p| p.data_dir().to_path_buf())
        .ok_or(LibraryError::NoDataDir)
}

impl Library {
    /// Open (creating if needed) the library in the platform data directory.
    pub fn open_default() -> Result<Self> {
        let dir = default_data_dir()?;
        Self::open_in(&dir)
    }

    /// Open (creating if needed) `library.db` and the waveform cache inside `dir`.
    pub fn open_in(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let cache_dir = dir.join("waveforms");
        std::fs::create_dir_all(&cache_dir)?;
        let conn = Connection::open(dir.join("library.db"))?;
        Self::with_connection(conn, cache_dir)
    }

    /// An in-memory library (for tests); waveforms go to `cache_dir`.
    pub fn open_in_memory(cache_dir: PathBuf) -> Result<Self> {
        Self::with_connection(Connection::open_in_memory()?, cache_dir)
    }

    fn with_connection(conn: Connection, cache_dir: PathBuf) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        schema::migrate(&conn)?;
        Ok(Self { conn, cache_dir })
    }

    /// Path of a track's waveform cache file.
    pub fn waveform_path(&self, id: djk_core::TrackId) -> PathBuf {
        self.cache_dir.join(format!("{}.wf", id.0))
    }

    /// Schema version (for tests and diagnostics).
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
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
