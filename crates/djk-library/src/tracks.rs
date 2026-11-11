//! Track queries, analysis records and missing-file detection.

use std::path::{Path, PathBuf};

use djk_core::{Beatgrid, Key, TrackId};
use rusqlite::{OptionalExtension, Row, params};

use crate::{Library, LibraryError, Result, file_stamp};

/// One track as shown in the library and set views.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackRow {
    pub id: TrackId,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub bpm: Option<f32>,
    pub key: Option<Key>,
    /// Integrated loudness in LUFS.
    pub lufs: Option<f32>,
    pub duration_secs: Option<f64>,
    pub missing: bool,
    pub analysed: bool,
}

/// Stored analysis results for a track.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisRecord {
    pub duration_secs: f64,
    /// Codec/container label, e.g. `MP3`, `FLAC`, `WAV`.
    pub format: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Average bitrate in kbps (`None` for lossless).
    pub bitrate_kbps: Option<u32>,
    pub lossless: bool,
    pub bpm: Option<f32>,
    pub key: Option<Key>,
    /// Integrated loudness in LUFS (EBU R128).
    pub lufs: Option<f32>,
    /// The detected beatgrid (a user-edited grid is stored separately and wins).
    pub grid: Option<Beatgrid>,
}

/// A track's beatgrid and whether the user edited it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoredGrid {
    pub grid: Beatgrid,
    /// True when the user adjusted it; re-analysis then leaves it alone.
    pub edited: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    Title,
    Artist,
    Bpm,
    Key,
    Lufs,
    Duration,
}

/// Search and sort options for [`Library::tracks`].
#[derive(Debug, Clone, Default)]
pub struct TrackQuery {
    /// Case-insensitive text matched against title and artist.
    pub search: String,
    pub sort: SortKey,
    pub descending: bool,
}

/// A user-edited grid's tempo takes precedence over the detected BPM.
const TRACK_COLUMNS: &str = "t.id, t.path, t.title, t.artist, COALESCE(t.user_grid_bpm, a.bpm), \
     a.key, a.duration_ms, t.missing, a.track_id IS NOT NULL, a.lufs";

pub(crate) fn track_from_row(r: &Row<'_>) -> rusqlite::Result<TrackRow> {
    let key: Option<i64> = r.get(5)?;
    let duration_ms: Option<i64> = r.get(6)?;
    Ok(TrackRow {
        id: TrackId(r.get(0)?),
        path: PathBuf::from(r.get::<_, String>(1)?),
        title: r.get(2)?,
        artist: r.get(3)?,
        bpm: r.get::<_, Option<f64>>(4)?.map(|b| b as f32),
        key: key.and_then(|k| Key::from_code(k as u8)),
        lufs: r.get::<_, Option<f64>>(9)?.map(|l| l as f32),
        duration_secs: duration_ms.map(|ms| ms as f64 / 1000.0),
        missing: r.get(7)?,
        analysed: r.get(8)?,
    })
}

/// A grid from its stored columns (tempo, first beat, bar phase), if all are present.
fn grid_from(bpm: Option<f64>, start: Option<f64>, downbeat: Option<i64>) -> Option<Beatgrid> {
    Some(Beatgrid::with_phase(bpm?, start?, downbeat.unwrap_or(0)))
}

pub(crate) fn select_tracks_sql(tail: &str) -> String {
    format!("SELECT {TRACK_COLUMNS} FROM tracks t LEFT JOIN analysis a ON a.track_id = t.id {tail}")
}

/// Sort rows in place; tracks without a value always sort last.
pub fn sort_tracks(rows: &mut [TrackRow], sort: SortKey, descending: bool) {
    use std::cmp::Ordering;
    fn opt<T: PartialOrd>(a: Option<T>, b: Option<T>, desc: bool) -> Ordering {
        match (a, b) {
            (Some(a), Some(b)) => {
                let o = a.partial_cmp(&b).unwrap_or(Ordering::Equal);
                if desc { o.reverse() } else { o }
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
    let text = |a: &str, b: &str, desc: bool| {
        let o = a.to_lowercase().cmp(&b.to_lowercase());
        if desc { o.reverse() } else { o }
    };
    rows.sort_by(|a, b| {
        let primary = match sort {
            SortKey::Title => text(&a.title, &b.title, descending),
            SortKey::Artist => text(&a.artist, &b.artist, descending),
            SortKey::Bpm => opt(a.bpm, b.bpm, descending),
            SortKey::Key => opt(
                a.key.map(|k| (k.camelot_number(), k.mode as u8)),
                b.key.map(|k| (k.camelot_number(), k.mode as u8)),
                descending,
            ),
            SortKey::Lufs => opt(a.lufs, b.lufs, descending),
            SortKey::Duration => opt(a.duration_secs, b.duration_secs, descending),
        };
        primary.then_with(|| text(&a.title, &b.title, false))
    });
}

/// Whether a track matches a case-insensitive search over title and artist.
pub fn matches_search(row: &TrackRow, needle_lower: &str) -> bool {
    needle_lower.is_empty()
        || row.title.to_lowercase().contains(needle_lower)
        || row.artist.to_lowercase().contains(needle_lower)
}

impl Library {
    /// All tracks matching the query, sorted.
    pub fn tracks(&self, query: &TrackQuery) -> Result<Vec<TrackRow>> {
        let mut stmt = self.conn.prepare_cached(&select_tracks_sql(""))?;
        let needle = query.search.trim().to_lowercase();
        let mut rows: Vec<TrackRow> = stmt
            .query_map([], track_from_row)?
            .filter_map(|r| r.ok())
            .filter(|r| matches_search(r, &needle))
            .collect();
        sort_tracks(&mut rows, query.sort, query.descending);
        Ok(rows)
    }

    pub fn track(&self, id: TrackId) -> Result<TrackRow> {
        self.conn
            .query_row(
                &select_tracks_sql("WHERE t.id = ?1"),
                [id.0],
                track_from_row,
            )
            .optional()?
            .ok_or(LibraryError::NotFound)
    }

    pub fn track_count(&self) -> Result<usize> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, i64>(0))?
            as usize)
    }

    /// Remove a track from the library: its row, its analysis and its place in every set.
    ///
    /// The audio file itself is never touched. The only file removed is djkitty's own
    /// cached waveform for the track, which lives in the library's data folder.
    /// Importing the file again adds it back.
    pub fn remove_track(&mut self, id: TrackId) -> Result<()> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", [id.0])?;
        let cache = self.waveform_path(id);
        debug_assert!(cache.starts_with(&self.cache_dir));
        let _ = std::fs::remove_file(cache);
        Ok(())
    }

    /// How many sets contain the track.
    pub fn sets_containing(&self, id: TrackId) -> Result<usize> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM set_tracks WHERE track_id = ?1",
            [id.0],
            |r| r.get::<_, i64>(0),
        )? as usize)
    }

    /// Re-check whether each track's file exists and update its `missing` flag. Returns
    /// the ids whose flag changed.
    pub fn refresh_missing(&mut self, ids: &[TrackId]) -> Result<Vec<TrackId>> {
        let mut changed = Vec::new();
        let tx = self.conn.transaction()?;
        {
            let mut get = tx.prepare_cached("SELECT path, missing FROM tracks WHERE id = ?1")?;
            let mut set = tx.prepare_cached("UPDATE tracks SET missing = ?2 WHERE id = ?1")?;
            for &id in ids {
                let Some((path, was_missing)) = get
                    .query_row([id.0], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
                    })
                    .optional()?
                else {
                    continue;
                };
                let missing = !Path::new(&path).is_file();
                if missing != was_missing {
                    set.execute(params![id.0, missing])?;
                    changed.push(id);
                }
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Path to load for a track, refusing (and flagging) missing files.
    pub fn loadable_path(&mut self, id: TrackId) -> Result<PathBuf> {
        let row = self.track(id)?;
        self.refresh_missing(&[id])?;
        if row.path.is_file() {
            Ok(row.path)
        } else {
            Err(LibraryError::Missing(row.path))
        }
    }

    /// Stored analysis for a track.
    pub fn analysis(&self, id: TrackId) -> Result<Option<AnalysisRecord>> {
        Ok(self
            .conn
            .query_row(
                "SELECT duration_ms, format, sample_rate, channels, bitrate, lossless, bpm, key, \
                        grid_bpm, grid_start, grid_downbeat, lufs \
                 FROM analysis WHERE track_id = ?1",
                [id.0],
                |r| {
                    let grid = grid_from(r.get(8)?, r.get(9)?, r.get(10)?);
                    Ok(AnalysisRecord {
                        duration_secs: r.get::<_, i64>(0)? as f64 / 1000.0,
                        format: r.get(1)?,
                        sample_rate: r.get(2)?,
                        channels: r.get(3)?,
                        bitrate_kbps: r.get(4)?,
                        lossless: r.get(5)?,
                        bpm: r.get::<_, Option<f64>>(6)?.map(|b| b as f32),
                        key: r
                            .get::<_, Option<i64>>(7)?
                            .and_then(|k| Key::from_code(k as u8)),
                        grid,
                        lufs: r.get::<_, Option<f64>>(11)?.map(|l| l as f32),
                    })
                },
            )
            .optional()?)
    }

    /// The track's beatgrid: the user's edited grid if there is one, otherwise the
    /// detected one. `None` until analysed (or when no beat was found).
    pub fn beatgrid(&self, id: TrackId) -> Result<Option<StoredGrid>> {
        type Cols = (
            Option<f64>,
            Option<f64>,
            Option<i64>,
            Option<f64>,
            Option<f64>,
            Option<i64>,
        );
        let row: Option<Cols> = self
            .conn
            .query_row(
                "SELECT t.user_grid_bpm, t.user_grid_start, t.user_grid_downbeat, \
                        a.grid_bpm, a.grid_start, a.grid_downbeat \
                 FROM tracks t LEFT JOIN analysis a ON a.track_id = t.id WHERE t.id = ?1",
                [id.0],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((ub, us, ud, db, ds, dd)) = row else {
            return Ok(None);
        };
        Ok(match (grid_from(ub, us, ud), grid_from(db, ds, dd)) {
            (Some(grid), _) => Some(StoredGrid { grid, edited: true }),
            (None, Some(grid)) => Some(StoredGrid {
                grid,
                edited: false,
            }),
            (None, None) => None,
        })
    }

    /// Save a user-adjusted beatgrid. It survives re-analysis until reset.
    pub fn set_beatgrid(&mut self, id: TrackId, grid: Beatgrid) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE tracks SET user_grid_bpm = ?2, user_grid_start = ?3, user_grid_downbeat = ?4 \
             WHERE id = ?1",
            params![id.0, grid.bpm, grid.start_secs, grid.first_downbeat],
        )?;
        if n == 0 {
            return Err(LibraryError::NotFound);
        }
        Ok(())
    }

    /// Drop the user's grid edits and go back to the detected grid.
    pub fn reset_beatgrid(&mut self, id: TrackId) -> Result<()> {
        self.conn.execute(
            "UPDATE tracks SET user_grid_bpm = NULL, user_grid_start = NULL, \
             user_grid_downbeat = NULL WHERE id = ?1",
            [id.0],
        )?;
        Ok(())
    }

    /// Store analysis results, recording the file stamp they were computed from.
    pub fn save_analysis(
        &mut self,
        id: TrackId,
        rec: &AnalysisRecord,
        stamp: (i64, i64),
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO analysis \
             (track_id, duration_ms, format, sample_rate, channels, bitrate, lossless, bpm, key, \
              source_size, source_mtime, version, grid_bpm, grid_start, grid_downbeat, \
              lufs) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                id.0,
                (rec.duration_secs * 1000.0).round() as i64,
                rec.format,
                rec.sample_rate,
                rec.channels,
                rec.bitrate_kbps,
                rec.lossless,
                rec.bpm.map(f64::from),
                rec.key.map(|k| i64::from(k.to_code())),
                stamp.0,
                stamp.1,
                crate::ANALYSIS_VERSION,
                rec.grid.map(|g| g.bpm),
                rec.grid.map(|g| g.start_secs),
                rec.grid.map(|g| g.first_downbeat),
                rec.lufs.map(f64::from),
            ],
        )?;
        self.conn.execute(
            "UPDATE tracks SET size = ?2, mtime = ?3, missing = 0 WHERE id = ?1",
            params![id.0, stamp.0, stamp.1],
        )?;
        Ok(())
    }

    /// Tracks that need (re-)analysis: never analysed, analysed by an older version of the
    /// analysis, or whose file changed size or modification time since. Missing files are
    /// flagged and skipped.
    pub fn tracks_needing_analysis(&mut self) -> Result<Vec<(TrackId, PathBuf)>> {
        type Row = (i64, String, Option<i64>, Option<i64>, Option<i64>);
        let rows: Vec<Row> = {
            let mut stmt = self.conn.prepare(
                "SELECT t.id, t.path, a.source_size, a.source_mtime, a.version \
                 FROM tracks t LEFT JOIN analysis a ON a.track_id = t.id ORDER BY t.id",
            )?;
            stmt.query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        let mut need = Vec::new();
        let mut missing = Vec::new();
        for (id, path, size, mtime, version) in rows {
            let path = PathBuf::from(path);
            match file_stamp(&path) {
                Ok(stamp) => {
                    let outdated = version.is_some_and(|v| v < crate::ANALYSIS_VERSION);
                    if size.zip(mtime) != Some(stamp) || outdated {
                        need.push((TrackId(id), path));
                    }
                }
                Err(_) => missing.push(TrackId(id)),
            }
        }
        self.refresh_missing(&missing)?;
        Ok(need)
    }

    /// Current file stamp of a path (size, mtime), as stored with analysis.
    pub fn stamp(path: &Path) -> Result<(i64, i64)> {
        Ok(file_stamp(path)?)
    }
}
