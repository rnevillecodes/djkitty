//! Sets: named, ordered lists of library tracks, each mirrored as an M3U8 playlist in the
//! vault's `Sets` folder.
//!
//! The database is the source of truth. A set's playlist file is rewritten after every
//! change to the set and whenever the vault opens; edits made to it elsewhere are
//! overwritten. `sets.file_name` records which file belongs to each set, and djkitty never
//! touches any other file in `Sets`. Failing to write a playlist file never fails the
//! change to the set: it becomes a warning (see [`Library::take_warnings`]).

use djk_core::TrackId;
use rusqlite::{OptionalExtension, params};

use crate::tracks::{TrackRow, select_tracks_sql, track_from_row};
use crate::{Library, LibraryError, Result, names};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SetId(pub i64);

/// A set as shown in the sets list.
#[derive(Debug, Clone, PartialEq)]
pub struct SetSummary {
    pub id: SetId,
    pub name: String,
    pub track_count: usize,
    /// Total duration of analysed tracks, in seconds.
    pub total_secs: f64,
}

const PLAYLIST_EXT: &str = ".m3u8";

impl Library {
    pub fn create_set(&mut self, name: &str) -> Result<SetId> {
        self.conn
            .execute("INSERT INTO sets (name) VALUES (?1)", [name.trim()])?;
        let id = SetId(self.conn.last_insert_rowid());
        self.write_set_file(id)?;
        Ok(id)
    }

    pub fn rename_set(&mut self, id: SetId, name: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE sets SET name = ?2 WHERE id = ?1",
            params![id.0, name.trim()],
        )?;
        if n == 0 {
            return Err(LibraryError::NotFound);
        }
        self.write_set_file(id)
    }

    /// Delete a set and its playlist file. Its tracks stay in the library.
    pub fn delete_set(&mut self, id: SetId) -> Result<()> {
        let file = self.set_file_name(id)?;
        self.conn
            .execute("DELETE FROM sets WHERE id = ?1", [id.0])?;
        if let Some(file) = file {
            self.remove_set_file(&file);
        }
        Ok(())
    }

    pub fn sets(&self) -> Result<Vec<SetSummary>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT s.id, s.name, COUNT(st.track_id), COALESCE(SUM(a.duration_ms), 0) \
             FROM sets s \
             LEFT JOIN set_tracks st ON st.set_id = s.id \
             LEFT JOIN analysis a ON a.track_id = st.track_id \
             GROUP BY s.id ORDER BY s.name COLLATE NOCASE, s.id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(SetSummary {
                    id: SetId(r.get(0)?),
                    name: r.get(1)?,
                    track_count: r.get::<_, i64>(2)? as usize,
                    total_secs: r.get::<_, i64>(3)? as f64 / 1000.0,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn set_summary(&self, id: SetId) -> Result<SetSummary> {
        self.sets()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or(LibraryError::NotFound)
    }

    /// Append a track to the end of a set. Returns false (and changes nothing) if the set
    /// already contains it.
    pub fn add_to_set(&mut self, set: SetId, track: TrackId) -> Result<bool> {
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO set_tracks (set_id, track_id, position) \
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position) + 1, 0) FROM set_tracks WHERE set_id = ?1))",
            params![set.0, track.0],
        )?;
        if n > 0 {
            self.write_set_file(set)?;
        }
        Ok(n > 0)
    }

    pub fn remove_from_set(&mut self, set: SetId, track: TrackId) -> Result<()> {
        let order: Vec<TrackId> = self
            .set_order(set)?
            .into_iter()
            .filter(|&t| t != track)
            .collect();
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM set_tracks WHERE set_id = ?1 AND track_id = ?2",
            params![set.0, track.0],
        )?;
        write_order(&tx, set, &order)?;
        tx.commit()?;
        self.write_set_file(set)
    }

    /// Move the track at index `from` to index `to` (0-based) within a set.
    pub fn move_in_set(&mut self, set: SetId, from: usize, to: usize) -> Result<()> {
        let mut order = self.set_order(set)?;
        if from >= order.len() {
            return Err(LibraryError::NotFound);
        }
        let t = order.remove(from);
        order.insert(to.min(order.len()), t);
        let tx = self.conn.transaction()?;
        write_order(&tx, set, &order)?;
        tx.commit()?;
        self.write_set_file(set)
    }

    fn set_order(&self, set: SetId) -> Result<Vec<TrackId>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT track_id FROM set_tracks WHERE set_id = ?1 ORDER BY position",
        )?;
        let ids = stmt
            .query_map([set.0], |r| Ok(TrackId(r.get(0)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// Tracks of a set in order.
    pub fn set_tracks(&self, set: SetId) -> Result<Vec<TrackRow>> {
        let mut stmt = self.conn.prepare_cached(&select_tracks_sql(
            "JOIN set_tracks st ON st.track_id = t.id WHERE st.set_id = ?1 ORDER BY st.position",
        ))?;
        let rows = stmt
            .query_map([set.0], |r| track_from_row(&self.vault, r))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn set_exists(&self, set: SetId) -> Result<bool> {
        Ok(self
            .conn
            .query_row("SELECT 1 FROM sets WHERE id = ?1", [set.0], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Sets that contain a track.
    pub(crate) fn sets_with_track(&self, track: TrackId) -> Result<Vec<SetId>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT set_id FROM set_tracks WHERE track_id = ?1")?;
        let ids = stmt
            .query_map([track.0], |r| Ok(SetId(r.get(0)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// The playlist file name of a set, relative to `Sets` (`None` until first written).
    pub fn set_file_name(&self, id: SetId) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT file_name FROM sets WHERE id = ?1", [id.0], |r| {
                r.get(0)
            })
            .optional()?
            .flatten())
    }

    /// Rewrite every set's playlist file (when the vault opens).
    pub(crate) fn write_all_set_files(&mut self) -> Result<()> {
        let ids: Vec<SetId> = {
            let mut stmt = self.conn.prepare("SELECT id FROM sets ORDER BY id")?;
            stmt.query_map([], |r| Ok(SetId(r.get(0)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in ids {
            self.write_set_file(id)?;
        }
        Ok(())
    }

    /// Bring a set's playlist file up to date: pick its file name (renaming the old file
    /// when the set's name changed) and write its tracks. Database errors are returned;
    /// file errors become warnings.
    pub(crate) fn write_set_file(&mut self, id: SetId) -> Result<()> {
        let Some((name, current)): Option<(String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT name, file_name FROM sets WHERE id = ?1",
                [id.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
        else {
            return Ok(());
        };
        let mut wanted = names::sanitize_component(&name);
        if wanted.is_empty() {
            wanted = "Untitled set".to_owned();
        }
        let wanted = names::sanitize_component(&format!("{wanted}{PLAYLIST_EXT}"));
        let keep = current.as_deref().filter(|c| names::base_name(c) == wanted);
        let file = match keep {
            Some(c) => c.to_owned(),
            None => {
                let sets_dir = self.vault.sets_dir();
                let mut stmt = self.conn.prepare_cached(
                    "SELECT EXISTS (SELECT 1 FROM sets WHERE file_name = ?1 AND id != ?2)",
                )?;
                let mut db_error = None;
                let file = names::unique_name(&wanted, |candidate| {
                    let owned = stmt
                        .query_row(params![candidate, id.0], |r| r.get::<_, bool>(0))
                        .unwrap_or_else(|e| {
                            db_error = Some(e);
                            true
                        });
                    let is_current = current
                        .as_deref()
                        .is_some_and(|c| c.eq_ignore_ascii_case(candidate));
                    owned || (!is_current && sets_dir.join(candidate).exists())
                });
                if let Some(e) = db_error {
                    return Err(e.into());
                }
                file
            }
        };

        let tracks = self.set_tracks(id)?;
        let contents = playlist(&tracks, self.vault.root());
        let dir = self.vault.sets_dir();
        let tmp = dir.join(format!(".{file}.tmp"));
        let written = std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&tmp, contents))
            .and_then(|()| std::fs::rename(&tmp, dir.join(&file)));
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            self.warn(format!(
                "Could not write the playlist file for set “{name}”: {e}"
            ));
            return Ok(());
        }
        if current.as_deref() != Some(file.as_str()) {
            self.conn.execute(
                "UPDATE sets SET file_name = ?2 WHERE id = ?1",
                params![id.0, file],
            )?;
            if let Some(old) = current
                && !old.eq_ignore_ascii_case(&file)
            {
                self.remove_set_file(&old);
            }
        }
        Ok(())
    }

    /// Delete a playlist file djkitty owns (a bare file name inside `Sets`).
    fn remove_set_file(&mut self, file: &str) {
        if file.is_empty() || file.contains(['/', '\\', ':']) || file == "." || file == ".." {
            tracing::warn!("not deleting set file {file:?}: not a plain file name");
            return;
        }
        let path = self.vault.sets_dir().join(file);
        if let Err(e) = std::fs::remove_file(&path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            self.warn(format!("Could not delete the playlist file {file}: {e}"));
        }
    }
}

/// M3U8 contents for a set: one `#EXTINF` line and one path per track, the paths relative
/// to the `Sets` folder with `/` separators.
fn playlist(tracks: &[TrackRow], root: &std::path::Path) -> String {
    let mut out = String::from("#EXTM3U\n");
    for t in tracks {
        let secs = t.duration_secs.map_or(-1, |s| s.round() as i64);
        let label = if t.artist.is_empty() {
            t.title.clone()
        } else {
            format!("{} - {}", t.artist, t.title)
        };
        let rel = t
            .path
            .strip_prefix(root)
            .map(|p| {
                p.components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_else(|_| t.path.to_string_lossy().into_owned());
        // A newline inside a tag would break the line structure.
        let label = label.replace(['\r', '\n'], " ");
        out.push_str(&format!("#EXTINF:{secs},{label}\n../{rel}\n"));
    }
    out
}

fn write_order(tx: &rusqlite::Transaction<'_>, set: SetId, order: &[TrackId]) -> Result<()> {
    let mut stmt = tx.prepare_cached(
        "UPDATE set_tracks SET position = ?3 WHERE set_id = ?1 AND track_id = ?2",
    )?;
    for (pos, t) in order.iter().enumerate() {
        stmt.execute(params![set.0, t.0, pos as i64])?;
    }
    Ok(())
}
