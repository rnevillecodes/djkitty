//! File operations on audio: hashing, copying into the vault and deleting from the
//! Catalog. Originals are only ever opened for reading; [`Library::delete_catalog_file`]
//! is the only code that deletes audio, and only inside the vault's Catalog.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Library;

const BUF: usize = 1 << 20;

/// BLAKE3 hash of a file's bytes, as lowercase hex.
pub(crate) fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; BUF];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Which side of a copy failed: a source error only affects that file, a destination
/// error (e.g. the vault's disk is full) affects everything after it.
#[derive(Debug)]
pub(crate) enum CopyError {
    Source(std::io::Error),
    Dest(std::io::Error),
}

/// Copy `src` to a new file `dst`, hashing what was read, and flush it to disk. Returns
/// the hash and the number of bytes copied. On error `dst` is removed.
pub(crate) fn copy_hashing(src: &Path, dst: &Path) -> Result<(String, u64), CopyError> {
    let result = (|| {
        let mut input = File::open(src).map_err(CopyError::Source)?;
        let mut output = File::create_new(dst).map_err(CopyError::Dest)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; BUF];
        let mut total = 0u64;
        loop {
            let n = input.read(&mut buf).map_err(CopyError::Source)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            output.write_all(&buf[..n]).map_err(CopyError::Dest)?;
            total += n as u64;
        }
        output.sync_all().map_err(CopyError::Dest)?;
        Ok((hasher.finalize().to_hex().to_string(), total))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(dst);
    }
    result
}

/// What happened to a removed track's Catalog file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRemoval {
    /// The vault copy was deleted.
    Deleted,
    /// There was no file to delete (it was already missing).
    AlreadyGone,
    /// The stored location is not inside the Catalog, so nothing was deleted.
    Refused,
    /// Deleting failed (e.g. the file is open elsewhere); the reason.
    Failed(String),
}

static PART_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Library {
    /// A fresh path in the vault's `incoming` folder for a copy in progress.
    pub(crate) fn new_part_path(&self) -> PathBuf {
        let n = PART_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        self.vault
            .incoming_dir()
            .join(format!("{}-{nanos}-{n}.part", std::process::id()))
    }

    /// Delete the Catalog file at vault-relative `rel`, but only if its real location
    /// (links and `..` resolved) is a file inside the vault's Catalog. An artist folder
    /// left empty is removed too.
    pub(crate) fn delete_catalog_file(&self, rel: &str) -> FileRemoval {
        let path = self.vault.resolve(rel);
        let real = match std::fs::canonicalize(&path) {
            Ok(p) => p,
            Err(_) if !path.exists() => return FileRemoval::AlreadyGone,
            Err(e) => return FileRemoval::Failed(e.to_string()),
        };
        let Ok(catalog) = std::fs::canonicalize(self.vault.catalog_dir()) else {
            tracing::warn!("cannot resolve the Catalog folder; not deleting {rel}");
            return FileRemoval::Refused;
        };
        if real == catalog || !real.starts_with(&catalog) || !real.is_file() {
            tracing::warn!(
                "refusing to delete {} (stored as {rel}): not a file inside the Catalog",
                real.display()
            );
            return FileRemoval::Refused;
        }
        if let Err(e) = std::fs::remove_file(&real) {
            return FileRemoval::Failed(e.to_string());
        }
        if let Some(dir) = real.parent()
            && dir != catalog
            && dir.starts_with(&catalog)
        {
            // Fails (harmlessly) unless the folder is empty.
            let _ = std::fs::remove_dir(dir);
        }
        FileRemoval::Deleted
    }
}
