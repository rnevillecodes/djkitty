//! Schema migrations, tracked with `PRAGMA user_version`.
//!
//! Vault libraries started over at v1 (the old global library is not migrated).

use rusqlite::Connection;

/// Each entry upgrades the schema from version `index` to `index + 1`.
const MIGRATIONS: &[&str] = &[
    // v1: a vault library. Tracks are identified by the BLAKE3 hash of their file and
    // stored at a vault-relative `path` (always under `Catalog/`). `source_*` record
    // where a track was imported from, so re-importing an unchanged file needs no read.
    // A user-edited beatgrid lives on the track, so re-analysis never overwrites it; the
    // detected grid lives with the analysis. `sets.file_name` is the playlist file in
    // `Sets/` that djkitty owns for the set.
    r#"
    CREATE TABLE tracks (
        id                 INTEGER PRIMARY KEY AUTOINCREMENT,
        hash               TEXT NOT NULL UNIQUE,
        path               TEXT NOT NULL UNIQUE COLLATE NOCASE,
        title              TEXT NOT NULL,
        artist             TEXT NOT NULL DEFAULT '',
        size               INTEGER NOT NULL DEFAULT 0,
        mtime              INTEGER NOT NULL DEFAULT 0,
        missing            INTEGER NOT NULL DEFAULT 0,
        added_at           INTEGER NOT NULL DEFAULT (unixepoch()),
        source_path        TEXT,
        source_size        INTEGER,
        source_mtime       INTEGER,
        user_grid_bpm      REAL,
        user_grid_start    REAL,
        user_grid_downbeat INTEGER
    );
    CREATE INDEX tracks_source ON tracks(source_path);
    CREATE TABLE analysis (
        track_id      INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
        duration_ms   INTEGER NOT NULL,
        format        TEXT NOT NULL,
        sample_rate   INTEGER NOT NULL,
        channels      INTEGER NOT NULL,
        bitrate       INTEGER,
        lossless      INTEGER NOT NULL DEFAULT 0,
        bpm           REAL,
        key           INTEGER,
        lufs          REAL,
        grid_bpm      REAL,
        grid_start    REAL,
        grid_downbeat INTEGER,
        source_size   INTEGER NOT NULL,
        source_mtime  INTEGER NOT NULL,
        version       INTEGER NOT NULL DEFAULT 1,
        analysed_at   INTEGER NOT NULL DEFAULT (unixepoch())
    );
    CREATE TABLE sets (
        id        INTEGER PRIMARY KEY,
        name      TEXT NOT NULL,
        file_name TEXT UNIQUE COLLATE NOCASE
    );
    CREATE TABLE set_tracks (
        set_id   INTEGER NOT NULL REFERENCES sets(id) ON DELETE CASCADE,
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        UNIQUE (set_id, track_id)
    );
    CREATE INDEX set_tracks_order ON set_tracks(set_id, position);
    CREATE TABLE hot_cues (
        track_id      INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        slot          INTEGER NOT NULL CHECK (slot BETWEEN 0 AND 7),
        position_secs REAL NOT NULL,
        UNIQUE (track_id, slot)
    );
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
