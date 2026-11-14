//! Import: walk files and folders, copy each new audio file into the vault's Catalog and
//! add it to the library.
//!
//! For each supported file: (1) skip it if this exact source (path, size, mtime) was
//! imported before; (2) hash it and skip it if the vault already holds that content;
//! (3) copy it into `.djkitty/incoming` while hashing again, and skip it if it changed;
//! (4) move it to `Catalog/<artist>/<name>`; (5) add the track and commit. A crash at
//! any point leaves complete tracks only.

use std::path::{Path, PathBuf};

use djk_core::TrackId;

use lofty::prelude::*;
use rusqlite::{OptionalExtension, params};

use crate::files::{CopyError, copy_hashing, hash_file};
use crate::vault::CATALOG_DIR;
use crate::{Library, LibraryError, Result, file_stamp, names};

/// Outcome of an import.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    /// New tracks added to the library (and copied into the vault).
    pub added: usize,
    /// Supported files already in the vault (left unchanged, nothing copied).
    pub existing: usize,
    /// Supported files that could not be read as audio, could not be read, or changed
    /// while they were being copied.
    pub skipped: usize,
    /// Files without a supported extension (ignored).
    pub ignored: usize,
    /// Bytes copied into the vault.
    pub bytes_copied: u64,
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
    /// Import every supported audio file under `dir` (recursively). See
    /// [`Library::import_paths`].
    pub fn import_folder(
        &mut self,
        dir: &Path,
        progress: impl FnMut(usize),
    ) -> Result<ImportSummary> {
        self.import_paths(&[dir.to_path_buf()], progress)
    }

    /// Import a mix of files and folders (folders recursively), e.g. from drag and drop,
    /// copying each new file into the vault's Catalog. Files whose content is already in
    /// the vault add nothing. `progress` is called with the number of files examined so
    /// far.
    ///
    /// Returns an error, keeping every track finished before it, when the vault itself
    /// can't be written (e.g. its disk is full) or the database fails.
    pub fn import_paths(
        &mut self,
        paths: &[PathBuf],
        progress: impl FnMut(usize),
    ) -> Result<ImportSummary> {
        self.import_paths_with_hook(paths, progress, |_| {})
    }

    /// [`Library::import_paths`] with a hook called for each new file after it was hashed
    /// and before it is copied (tests use it to change a file mid-import).
    #[doc(hidden)]
    pub fn import_paths_with_hook(
        &mut self,
        paths: &[PathBuf],
        mut progress: impl FnMut(usize),
        mut before_copy: impl FnMut(&Path),
    ) -> Result<ImportSummary> {
        let mut summary = ImportSummary::default();
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
            match self.import_one(path, &mut before_copy)? {
                Imported::New(id, bytes) => {
                    summary.added += 1;
                    summary.bytes_copied += bytes;
                    summary.tracks.push(id);
                }
                Imported::Existing(id) => {
                    if !summary.tracks.contains(&id) {
                        summary.existing += 1;
                        summary.tracks.push(id);
                    }
                }
                Imported::Skipped => summary.skipped += 1,
            }
        }
        Ok(summary)
    }

    fn import_one(&mut self, path: &Path, before_copy: &mut impl FnMut(&Path)) -> Result<Imported> {
        let source = path.to_string_lossy().into_owned();
        let Ok((size, mtime)) = file_stamp(path) else {
            return Ok(Imported::Skipped);
        };
        // (1) This exact file was imported before.
        if let Some(id) = self
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE source_path = ?1 AND source_size = ?2 \
                 AND source_mtime = ?3",
                params![source, size, mtime],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
        {
            return Ok(Imported::Existing(TrackId(id)));
        }
        // (2) The same content is already in the vault.
        let Ok(hash) = hash_file(path) else {
            tracing::info!("skipping unreadable file {}", path.display());
            return Ok(Imported::Skipped);
        };
        if let Some(id) = self.track_by_hash(&hash)? {
            return Ok(Imported::Existing(id));
        }
        if djk_analysis::probe_file(path).is_err() {
            tracing::info!("skipping undecodable file {}", path.display());
            return Ok(Imported::Skipped);
        }
        let (title, artist) = read_tags(path);
        before_copy(path);

        // (3) Copy, checking the content didn't change since it was hashed.
        let part = self.new_part_path();
        let copied = match copy_hashing(path, &part) {
            Ok((h, bytes)) if h == hash => bytes,
            Ok(_) => {
                tracing::info!("{} changed while importing; skipped", path.display());
                let _ = std::fs::remove_file(&part);
                return Ok(Imported::Skipped);
            }
            Err(CopyError::Source(e)) => {
                tracing::info!("cannot read {}: {e}; skipped", path.display());
                return Ok(Imported::Skipped);
            }
            Err(CopyError::Dest(e)) => return Err(LibraryError::VaultWrite(e)),
        };

        // (4) Move into the Catalog under a free name.
        let folder = names::artist_folder(&artist);
        let file = names::file_name(&path.file_name().unwrap_or_default().to_string_lossy());
        let rel = self.free_catalog_path(&folder, &file)?;
        let dest = self.vault.resolve(&rel);
        let placed = dest
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::rename(&part, &dest));
        if let Err(e) = placed {
            let _ = std::fs::remove_file(&part);
            return Err(LibraryError::VaultWrite(e));
        }

        // (5) Add the track. Without the row the copy must not stay.
        let (vsize, vmtime) = file_stamp(&dest).unwrap_or((copied as i64, 0));
        let inserted = self.conn.execute(
            "INSERT INTO tracks (hash, path, title, artist, size, mtime, source_path, \
             source_size, source_mtime) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![hash, rel, title, artist, vsize, vmtime, source, size, mtime],
        );
        if let Err(e) = inserted {
            let _ = std::fs::remove_file(&dest);
            if let Some(dir) = dest.parent() {
                let _ = std::fs::remove_dir(dir);
            }
            return Err(e.into());
        }
        Ok(Imported::New(
            TrackId(self.conn.last_insert_rowid()),
            copied,
        ))
    }

    pub(crate) fn track_by_hash(&self, hash: &str) -> Result<Option<TrackId>> {
        Ok(self
            .conn
            .query_row("SELECT id FROM tracks WHERE hash = ?1", [hash], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?
            .map(TrackId))
    }

    /// A vault-relative `Catalog/<folder>/<file>` path no track and no file uses yet
    /// (ignoring letter case), adding ` (2)`, ` (3)`... when needed.
    fn free_catalog_path(&self, folder: &str, file: &str) -> Result<String> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT EXISTS (SELECT 1 FROM tracks WHERE path = ?1)")?;
        let mut db_error = None;
        let name = names::unique_name(file, |candidate| {
            let rel = format!("{CATALOG_DIR}/{folder}/{candidate}");
            let in_db = stmt
                .query_row([&rel], |r| r.get::<_, bool>(0))
                .unwrap_or_else(|e| {
                    db_error = Some(e);
                    true
                });
            in_db || self.vault.resolve(&rel).exists() || db_error.is_some()
        });
        if let Some(e) = db_error {
            return Err(e.into());
        }
        Ok(format!("{CATALOG_DIR}/{folder}/{name}"))
    }
}

enum Imported {
    New(TrackId, u64),
    Existing(TrackId),
    Skipped,
}
