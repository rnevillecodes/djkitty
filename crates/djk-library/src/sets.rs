//! Sets: named, ordered lists of library tracks.

use djk_core::TrackId;
use rusqlite::{OptionalExtension, params};

use crate::tracks::{TrackRow, select_tracks_sql, track_from_row};
use crate::{Library, LibraryError, Result};

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

impl Library {
    pub fn create_set(&mut self, name: &str) -> Result<SetId> {
        self.conn
            .execute("INSERT INTO sets (name) VALUES (?1)", [name.trim()])?;
        Ok(SetId(self.conn.last_insert_rowid()))
    }

    pub fn rename_set(&mut self, id: SetId, name: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE sets SET name = ?2 WHERE id = ?1",
            params![id.0, name.trim()],
        )?;
        if n == 0 {
            return Err(LibraryError::NotFound);
        }
        Ok(())
    }

    /// Delete a set. Its tracks stay in the library.
    pub fn delete_set(&mut self, id: SetId) -> Result<()> {
        self.conn
            .execute("DELETE FROM sets WHERE id = ?1", [id.0])?;
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
        Ok(())
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
        Ok(())
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
            .query_map([set.0], track_from_row)?
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
