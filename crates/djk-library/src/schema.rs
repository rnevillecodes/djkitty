//! Schema migrations, tracked with `PRAGMA user_version`.

use rusqlite::Connection;

/// Each entry upgrades the schema from version `index` to `index + 1`.
const MIGRATIONS: &[&str] = &[
    // v1: tracks, analysis, sets.
    r#"
    CREATE TABLE tracks (
        id       INTEGER PRIMARY KEY,
        path     TEXT NOT NULL UNIQUE,
        title    TEXT NOT NULL,
        artist   TEXT NOT NULL DEFAULT '',
        size     INTEGER NOT NULL DEFAULT 0,
        mtime    INTEGER NOT NULL DEFAULT 0,
        missing  INTEGER NOT NULL DEFAULT 0,
        added_at INTEGER NOT NULL DEFAULT (unixepoch())
    );
    CREATE TABLE analysis (
        track_id     INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
        duration_ms  INTEGER NOT NULL,
        format       TEXT NOT NULL,
        sample_rate  INTEGER NOT NULL,
        channels     INTEGER NOT NULL,
        bitrate      INTEGER,
        lossless     INTEGER NOT NULL DEFAULT 0,
        bpm          REAL,
        key          INTEGER,
        source_size  INTEGER NOT NULL,
        source_mtime INTEGER NOT NULL,
        analysed_at  INTEGER NOT NULL DEFAULT (unixepoch())
    );
    CREATE TABLE sets (
        id   INTEGER PRIMARY KEY,
        name TEXT NOT NULL
    );
    CREATE TABLE set_tracks (
        set_id   INTEGER NOT NULL REFERENCES sets(id) ON DELETE CASCADE,
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        UNIQUE (set_id, track_id)
    );
    CREATE INDEX set_tracks_order ON set_tracks(set_id, position);
    "#,
    // v2: which version of the analysis produced each result, so improved analysis
    // (e.g. better BPM detection) re-runs on existing tracks.
    r#"
    ALTER TABLE analysis ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
    "#,
    // v3: beatgrids, first version (tempo and an anchor downbeat). Superseded by v4;
    // `grid_anchor` and `user_grid_anchor` are no longer used. Kept as-is because
    // libraries already at v3 must not have it change under them.
    r#"
    ALTER TABLE analysis ADD COLUMN grid_bpm REAL;
    ALTER TABLE analysis ADD COLUMN grid_anchor REAL;
    ALTER TABLE tracks ADD COLUMN user_grid_bpm REAL;
    ALTER TABLE tracks ADD COLUMN user_grid_anchor REAL;
    "#,
    // v4: beatgrids start at their first beat and record which beat starts bar 1. The
    // detected grid lives with the analysis (replaced on re-analysis); the user's edited
    // grid lives on the track, so re-analysis never overwrites it.
    r#"
    ALTER TABLE analysis ADD COLUMN grid_start REAL;
    ALTER TABLE analysis ADD COLUMN grid_downbeat INTEGER;
    ALTER TABLE tracks ADD COLUMN user_grid_start REAL;
    ALTER TABLE tracks ADD COLUMN user_grid_downbeat INTEGER;
    -- Old-format grid edits had no start or bar phase; drop them so the detected grid
    -- shows (none were made outside development).
    UPDATE tracks SET user_grid_bpm = NULL;
    "#,
    // v5: integrated loudness (LUFS).
    r#"
    ALTER TABLE analysis ADD COLUMN lufs REAL;
    "#,
    // v6: hot cues, one row per set slot, in track seconds.
    r#"
    CREATE TABLE hot_cues (
        track_id      INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        slot          INTEGER NOT NULL CHECK (slot BETWEEN 0 AND 7),
        position_secs REAL NOT NULL,
        UNIQUE (track_id, slot)
    );
    "#,
    // v7: AUTOINCREMENT track ids, so a removed track's id is never given to a later
    // import (the app keys decks, hot cue saves and analysis by id). SQLite can't add it
    // in place, so the table is rebuilt; `migrate` turns foreign keys off around this so
    // dropping the old table doesn't cascade into analysis, sets and hot cues.
    r#"
    CREATE TABLE tracks_new (
        id                 INTEGER PRIMARY KEY AUTOINCREMENT,
        path               TEXT NOT NULL UNIQUE,
        title              TEXT NOT NULL,
        artist             TEXT NOT NULL DEFAULT '',
        size               INTEGER NOT NULL DEFAULT 0,
        mtime              INTEGER NOT NULL DEFAULT 0,
        missing            INTEGER NOT NULL DEFAULT 0,
        added_at           INTEGER NOT NULL DEFAULT (unixepoch()),
        user_grid_bpm      REAL,
        user_grid_anchor   REAL,
        user_grid_start    REAL,
        user_grid_downbeat INTEGER
    );
    INSERT INTO tracks_new
        (id, path, title, artist, size, mtime, missing, added_at,
         user_grid_bpm, user_grid_anchor, user_grid_start, user_grid_downbeat)
    SELECT id, path, title, artist, size, mtime, missing, added_at,
           user_grid_bpm, user_grid_anchor, user_grid_start, user_grid_downbeat
    FROM tracks;
    DROP TABLE tracks;
    ALTER TABLE tracks_new RENAME TO tracks;
    "#,
];

/// Apply any migrations newer than the database's `user_version`.
///
/// Foreign keys are off while migrating (the pragma can't change inside a transaction),
/// so a migration can rebuild a referenced table; each one is checked before it commits.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current >= LATEST {
        return Ok(());
    }
    let fk: bool = conn.pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
    conn.pragma_update(None, "foreign_keys", false)?;
    let result = (|| {
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current.max(0) as usize) {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            let broken: i64 =
                tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
            if broken > 0 {
                // Rolls back when `tx` drops.
                return Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                    Some(format!("migration {} broke {broken} foreign key(s)", i + 1)),
                ));
            }
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    })();
    conn.pragma_update(None, "foreign_keys", fk)?;
    result
}

pub const LATEST: i64 = MIGRATIONS.len() as i64;
