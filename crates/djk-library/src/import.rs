//! Import: walk files and folders, filter, read tags, insert new tracks.

use std::path::{Path, PathBuf};

use djk_core::TrackId;

use lofty::prelude::*;
use rusqlite::{OptionalExtension, params};

use crate::{Library, Result, file_stamp};

/// Outcome of an import.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    /// New tracks added to the library.
    pub added: usize,
    /// Supported files already in the library (left unchanged).
    pub existing: usize,
    /// Supported files that could not be read as audio.
    pub skipped: usize,
    /// Files without a supported extension (ignored).
    pub ignored: usize,
    /// Every imported track, new or already in the library, in import order (no
    /// duplicates). Used e.g. to add dropped files to a set.
    pub tracks: Vec<TrackId>,
}

/// Title and artist from embedded tags, falling back to the file name.
pub fn read_tags(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tags = lofty::read_from_path(path).ok();
    let tag = tags
        .as_ref()
        .and_then(|f| f.primary_tag().or_else(|| f.first_tag()));
    let title = tag
        .and_then(|t| t.title())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or(stem);
    let artist = tag
        .and_then(|t| t.artist())
        .map(|s| s.trim().to_owned())
        .unwrap_or_default();
    (title, artist)
}

impl Library {
    /// Import every supported audio file under `dir` (recursively). Files already in the
    /// library (same path) are left unchanged. `progress` is called with the number of
    /// files examined so far.
    pub fn import_folder(
        &mut self,
        dir: &Path,
        progress: impl FnMut(usize),
    ) -> Result<ImportSummary> {
        self.import_paths(&[dir.to_path_buf()], progress)
    }

    /// Import a mix of files and folders (folders recursively), e.g. from drag and drop.
    /// Files already in the library (same path) are left unchanged. `progress` is called
    /// with the number of files examined so far.
    pub fn import_paths(
        &mut self,
        paths: &[PathBuf],
        mut progress: impl FnMut(usize),
    ) -> Result<ImportSummary> {
        let mut summary = ImportSummary::default();
        let tx = self.conn.transaction()?;
        {
            let mut existing_id = tx.prepare("SELECT id FROM tracks WHERE path = ?1")?;
            let mut insert = tx.prepare(
                "INSERT INTO tracks (path, title, artist, size, mtime) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            let mut seen = 0usize;
            let entries = paths.iter().flat_map(|p| {
                walkdir::WalkDir::new(p)
                    .follow_links(true)
                    .sort_by_file_name()
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
            });
            for entry in entries {
                seen += 1;
                progress(seen);
                let path = entry.path();
                if !djk_analysis::is_supported(path) {
                    summary.ignored += 1;
                    continue;
                }
                let path_str = path.to_string_lossy();
                if let Some(id) = existing_id
                    .query_row([path_str.as_ref()], |r| r.get::<_, i64>(0))
                    .optional()?
                {
                    let id = TrackId(id);
                    if !summary.tracks.contains(&id) {
                        summary.existing += 1;
                        summary.tracks.push(id);
                    }
                    continue;
                }
                if djk_analysis::probe_file(path).is_err() {
                    tracing::info!("skipping unreadable file {}", path.display());
                    summary.skipped += 1;
                    continue;
                }
                let (title, artist) = read_tags(path);
                let (size, mtime) = file_stamp(path)?;
                insert.execute(params![path_str.as_ref(), title, artist, size, mtime])?;
                summary.added += 1;
                summary.tracks.push(TrackId(tx.last_insert_rowid()));
            }
        }
        tx.commit()?;
        Ok(summary)
    }
}
