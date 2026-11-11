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
];

/// Apply any migrations newer than the database's `user_version`.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current.max(0) as usize) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

pub const LATEST: i64 = MIGRATIONS.len() as i64;
