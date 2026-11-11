//! Library tests covering the `track-library` and `set-management` spec scenarios.

use std::fs;
use std::path::{Path, PathBuf};

use djk_core::{Beatgrid, Key, Mode, TrackId};
use djk_library::{AnalysisRecord, Library, SCHEMA_VERSION, SortKey, TrackQuery};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../djk-analysis/tests/fixtures")
        .join(name)
}

fn copy(name: &str, to: &Path) {
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::copy(fixture(name), to).unwrap();
}

fn record(secs: f64, bpm: Option<f32>, key: Option<Key>) -> AnalysisRecord {
    AnalysisRecord {
        duration_secs: secs,
        format: "MP3".into(),
        sample_rate: 44_100,
        channels: 2,
        bitrate_kbps: Some(320),
        lossless: false,
        bpm,
        key,
        grid: None,
    }
}

fn library(dir: &Path) -> Library {
    Library::open_in(&dir.join("data")).unwrap()
}

fn ids(lib: &Library) -> Vec<TrackId> {
    lib.tracks(&TrackQuery::default())
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect()
}

#[test]
fn fresh_db_migrates_and_reopen_keeps_data() {
    let tmp = tempfile::tempdir().unwrap();
    copy("tone.mp3", &tmp.path().join("music/a.mp3"));
    {
        let mut lib = library(tmp.path());
        assert_eq!(lib.schema_version().unwrap(), SCHEMA_VERSION);
        lib.import_folder(&tmp.path().join("music"), |_| {})
            .unwrap();
        lib.create_set("Friday warmup").unwrap();
    }
    let lib = library(tmp.path());
    assert_eq!(lib.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(lib.track_count().unwrap(), 1);
    assert_eq!(lib.sets().unwrap()[0].name, "Friday warmup");
}

#[test]
fn imports_supported_files_recursively() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    for i in 0..3 {
        copy("tone.mp3", &music.join(format!("mp3/track{i}.mp3")));
    }
    copy("tone.flac", &music.join("flac/one.flac"));
    copy("tone.flac", &music.join("flac/deeper/two.flac"));
    let mut lib = library(tmp.path());
    let summary = lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(summary.added, 5);
    assert_eq!(lib.track_count().unwrap(), 5);
}

#[test]
fn unsupported_files_are_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("a.mp3"));
    fs::write(music.join("notes.txt"), "hello").unwrap();
    fs::write(music.join("cover.jpg"), [0xFF, 0xD8, 0xFF]).unwrap();
    let mut lib = library(tmp.path());
    let summary = lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(summary.added, 1);
    assert_eq!(summary.ignored, 2);
    assert_eq!(summary.skipped, 0);
}

#[test]
fn unreadable_audio_is_skipped_and_counted() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("good.mp3"));
    copy("corrupt.mp3", &music.join("bad.mp3"));
    copy("tone.wav", &music.join("good.wav"));
    let mut lib = library(tmp.path());
    let summary = lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(summary.added, 2);
    assert_eq!(summary.skipped, 1);
}

#[test]
fn reimport_adds_only_new_files() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("a.mp3"));
    copy("tone.mp3", &music.join("b.mp3"));
    let mut lib = library(tmp.path());
    lib.import_folder(&music, |_| {}).unwrap();
    let before = lib.tracks(&TrackQuery::default()).unwrap();

    copy("tone.flac", &music.join("c.flac"));
    let summary = lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(summary.added, 1);
    assert_eq!(summary.existing, 2);
    let after = lib.tracks(&TrackQuery::default()).unwrap();
    assert_eq!(after.len(), 3);
    for t in &before {
        assert!(after.contains(t), "existing track unchanged: {t:?}");
    }
}

#[test]
fn tags_are_read_with_filename_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("my_mix_track.mp3"));
    copy("tagged.mp3", &music.join("whatever.mp3"));
    let mut lib = library(tmp.path());
    lib.import_folder(&music, |_| {}).unwrap();
    let rows = lib.tracks(&TrackQuery::default()).unwrap();
    let untagged = rows
        .iter()
        .find(|r| r.path.ends_with("my_mix_track.mp3"))
        .unwrap();
    assert_eq!(untagged.title, "my_mix_track");
    assert_eq!(untagged.artist, "");
    let tagged = rows
        .iter()
        .find(|r| r.path.ends_with("whatever.mp3"))
        .unwrap();
    assert_eq!(tagged.title, "Sunrise Groove");
    assert_eq!(tagged.artist, "Ólafur Example");
}

fn library_with_titles(tmp: &Path) -> Library {
    let music = tmp.join("music");
    copy("tagged.mp3", &music.join("tagged.mp3"));
    for name in ["alpha", "Bravo", "charlie"] {
        copy("tone.mp3", &music.join(format!("{name}.mp3")));
    }
    let mut lib = library(tmp);
    lib.import_folder(&music, |_| {}).unwrap();
    lib
}

#[test]
fn search_is_case_insensitive_over_title_and_artist() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library_with_titles(tmp.path());
    let q = |s: &str| {
        lib.tracks(&TrackQuery {
            search: s.into(),
            ..Default::default()
        })
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect::<Vec<_>>()
    };
    assert_eq!(q("ÓLAF"), vec!["Sunrise Groove"]); // artist, non-ASCII case
    assert_eq!(q("bra"), vec!["Bravo"]);
    assert_eq!(q("SUNRISE"), vec!["Sunrise Groove"]);
    assert_eq!(q("").len(), 4);
    assert!(q("zzz").is_empty());
}

#[test]
fn sort_by_bpm_puts_unknown_last() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let rows = lib.tracks(&TrackQuery::default()).unwrap();
    let by_title = |t: &str| rows.iter().find(|r| r.title == t).unwrap().id;
    let stamp = (0, 0);
    lib.save_analysis(by_title("alpha"), &record(60.0, Some(128.0), None), stamp)
        .unwrap();
    lib.save_analysis(by_title("Bravo"), &record(60.0, Some(90.0), None), stamp)
        .unwrap();
    lib.save_analysis(by_title("charlie"), &record(60.0, None, None), stamp)
        .unwrap();
    for descending in [false, true] {
        let titles: Vec<String> = lib
            .tracks(&TrackQuery {
                sort: SortKey::Bpm,
                descending,
                ..Default::default()
            })
            .unwrap()
            .into_iter()
            .map(|t| t.title)
            .collect();
        let (first, second) = if descending {
            ("alpha", "Bravo")
        } else {
            ("Bravo", "alpha")
        };
        assert_eq!(&titles[..2], &[first, second], "descending={descending}");
        // Unknown BPM (charlie, unanalysed tagged track) last.
        assert!(titles[2..].contains(&"charlie".to_string()));
        assert!(titles[2..].contains(&"Sunrise Groove".to_string()));
    }
}

#[test]
fn sort_by_title_artist_key_and_duration() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let titles = |lib: &Library, sort| -> Vec<String> {
        lib.tracks(&TrackQuery {
            sort,
            ..Default::default()
        })
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect()
    };
    assert_eq!(
        titles(&lib, SortKey::Title),
        vec!["alpha", "Bravo", "charlie", "Sunrise Groove"]
    );
    assert_eq!(titles(&lib, SortKey::Artist)[3], "Sunrise Groove");
    let rows = lib.tracks(&TrackQuery::default()).unwrap();
    lib.save_analysis(
        rows[0].id,
        &record(300.0, None, Some(Key::new(9, Mode::Minor))), // 8A
        (0, 0),
    )
    .unwrap();
    lib.save_analysis(
        rows[1].id,
        &record(200.0, None, Some(Key::new(11, Mode::Major))), // 1B
        (0, 0),
    )
    .unwrap();
    assert_eq!(&titles(&lib, SortKey::Key)[..2], &["Bravo", "alpha"]);
    assert_eq!(&titles(&lib, SortKey::Duration)[..2], &["Bravo", "alpha"]);
}

#[test]
fn deleted_file_is_flagged_missing_and_cannot_load() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("gone.mp3"));
    let mut lib = library(tmp.path());
    lib.import_folder(&music, |_| {}).unwrap();
    let id = ids(&lib)[0];
    assert!(lib.loadable_path(id).is_ok());

    fs::remove_file(music.join("gone.mp3")).unwrap();
    let changed = lib.refresh_missing(&[id]).unwrap();
    assert_eq!(changed, vec![id]);
    assert!(lib.track(id).unwrap().missing);
    assert!(matches!(
        lib.loadable_path(id),
        Err(djk_library::LibraryError::Missing(_))
    ));
}

#[test]
fn analysis_needed_until_saved_and_again_after_file_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("a.mp3"));
    let mut lib = library(tmp.path());
    lib.import_folder(&music, |_| {}).unwrap();
    let need = lib.tracks_needing_analysis().unwrap();
    assert_eq!(need.len(), 1);
    let (id, path) = need[0].clone();
    let stamp = Library::stamp(&path).unwrap();
    lib.save_analysis(id, &record(1.0, Some(120.0), None), stamp)
        .unwrap();
    assert!(lib.tracks_needing_analysis().unwrap().is_empty());

    // Reopen: still analysed.
    drop(lib);
    let mut lib = library(tmp.path());
    assert!(lib.tracks_needing_analysis().unwrap().is_empty());
    assert_eq!(lib.analysis(id).unwrap().unwrap().bpm, Some(120.0));

    // Overwrite with a different file at the same path.
    copy("tone.flac", &path);
    assert_eq!(lib.tracks_needing_analysis().unwrap().len(), 1);
}

// ---- Sets ----

#[test]
fn create_rename_and_delete_sets() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let set = lib.create_set("Friday warmup").unwrap();
    let s = lib.set_summary(set).unwrap();
    assert_eq!((s.name.as_str(), s.track_count), ("Friday warmup", 0));

    lib.rename_set(set, "Saturday peak").unwrap();
    assert_eq!(lib.set_summary(set).unwrap().name, "Saturday peak");

    for id in ids(&lib) {
        lib.add_to_set(set, id).unwrap();
    }
    assert_eq!(lib.set_summary(set).unwrap().track_count, 4);
    lib.delete_set(set).unwrap();
    assert!(!lib.set_exists(set).unwrap());
    assert_eq!(lib.track_count().unwrap(), 4, "tracks remain in library");
}

#[test]
fn add_appends_and_duplicates_are_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let all = ids(&lib);
    let set = lib.create_set("A").unwrap();
    let other = lib.create_set("B").unwrap();
    assert!(lib.add_to_set(set, all[0]).unwrap());
    assert!(lib.add_to_set(set, all[1]).unwrap());
    assert!(!lib.add_to_set(set, all[0]).unwrap(), "duplicate");
    let order: Vec<_> = lib.set_tracks(set).unwrap().iter().map(|t| t.id).collect();
    assert_eq!(order, vec![all[0], all[1]]);
    // Allowed in more than one set.
    assert!(lib.add_to_set(other, all[0]).unwrap());
}

#[test]
fn remove_and_reorder_persist() {
    let tmp = tempfile::tempdir().unwrap();
    let all;
    let set;
    {
        let mut lib = library_with_titles(tmp.path());
        all = ids(&lib);
        set = lib.create_set("Order").unwrap();
        for &id in &all {
            lib.add_to_set(set, id).unwrap();
        }
        // Move the 4th track to position 2.
        lib.move_in_set(set, 3, 1).unwrap();
    }
    let mut lib = library(tmp.path());
    let order: Vec<_> = lib.set_tracks(set).unwrap().iter().map(|t| t.id).collect();
    assert_eq!(order, vec![all[0], all[3], all[1], all[2]]);

    lib.remove_from_set(set, all[3]).unwrap();
    let order: Vec<_> = lib.set_tracks(set).unwrap().iter().map(|t| t.id).collect();
    assert_eq!(order, vec![all[0], all[1], all[2]]);
    assert_eq!(lib.track_count().unwrap(), 4);
}

/// Byte-for-byte snapshot of every file under `dir`, to prove nothing was edited.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            out.extend(snapshot(&p));
        } else {
            out.push((p.clone(), fs::read(&p).unwrap()));
        }
    }
    out.sort();
    out
}

#[test]
fn results_from_an_older_analysis_version_are_redone() {
    let tmp = tempfile::tempdir().unwrap();
    let id;
    {
        let mut lib = library_with_titles(tmp.path());
        id = ids(&lib)[0];
        let path = lib.track(id).unwrap().path;
        let stamp = Library::stamp(&path).unwrap();
        lib.save_analysis(id, &record(60.0, Some(116.0), None), stamp)
            .unwrap();
        assert!(
            !lib.tracks_needing_analysis()
                .unwrap()
                .iter()
                .any(|(t, _)| *t == id)
        );
    }
    // Pretend the stored result came from analysis version 1.
    {
        let db = rusqlite::Connection::open(tmp.path().join("data/library.db")).unwrap();
        db.execute("UPDATE analysis SET version = 1", []).unwrap();
    }
    let mut lib = library(tmp.path());
    assert!(
        lib.tracks_needing_analysis()
            .unwrap()
            .iter()
            .any(|(t, _)| *t == id)
    );
}

#[test]
fn a_library_on_the_first_beatgrid_schema_upgrades() {
    // A library created at schema v3 (the first beatgrid columns) gets the v4 columns.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    std::fs::create_dir_all(&dir).unwrap();
    {
        let db = rusqlite::Connection::open(dir.join("library.db")).unwrap();
        db.execute_batch(
            "CREATE TABLE tracks (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, \
               title TEXT NOT NULL, artist TEXT NOT NULL DEFAULT '', size INTEGER NOT NULL DEFAULT 0, \
               mtime INTEGER NOT NULL DEFAULT 0, missing INTEGER NOT NULL DEFAULT 0, \
               added_at INTEGER NOT NULL DEFAULT 0, user_grid_bpm REAL, user_grid_anchor REAL);
             CREATE TABLE analysis (track_id INTEGER PRIMARY KEY, duration_ms INTEGER NOT NULL, \
               format TEXT NOT NULL, sample_rate INTEGER NOT NULL, channels INTEGER NOT NULL, \
               bitrate INTEGER, lossless INTEGER NOT NULL DEFAULT 0, bpm REAL, key INTEGER, \
               source_size INTEGER NOT NULL, source_mtime INTEGER NOT NULL, \
               analysed_at INTEGER NOT NULL DEFAULT 0, version INTEGER NOT NULL DEFAULT 1, \
               grid_bpm REAL, grid_anchor REAL);
             CREATE TABLE sets (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE set_tracks (set_id INTEGER NOT NULL, track_id INTEGER NOT NULL, \
               position INTEGER NOT NULL, UNIQUE (set_id, track_id));
             PRAGMA user_version = 3;",
        )
        .unwrap();
    }
    let mut lib = library(tmp.path());
    assert_eq!(lib.schema_version().unwrap(), SCHEMA_VERSION);
    let summary = {
        let music = tmp.path().join("music");
        copy("tone.mp3", &music.join("a.mp3"));
        lib.import_folder(&music, |_| {}).unwrap()
    };
    let id = summary.tracks[0];
    lib.set_beatgrid(id, Beatgrid::with_phase(128.0, 0.5, 1))
        .unwrap();
    assert_eq!(
        lib.beatgrid(id).unwrap().unwrap().grid,
        Beatgrid::with_phase(128.0, 0.5, 1)
    );
}

#[test]
fn edited_beatgrids_win_and_survive_reanalysis() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let id = ids(&lib)[0];
    assert_eq!(lib.beatgrid(id).unwrap(), None);

    // Analysis stores the detected grid.
    let detected = Beatgrid::new(174.0, 0.21);
    let mut rec = record(60.0, Some(174.0), None);
    rec.grid = Some(detected);
    lib.save_analysis(id, &rec, (0, 0)).unwrap();
    let g = lib.beatgrid(id).unwrap().unwrap();
    assert_eq!((g.grid, g.edited), (detected, false));

    // The user moves bar 1 and fine-tunes the tempo.
    let edited = Beatgrid::with_phase(174.05, 0.3, 2);
    lib.set_beatgrid(id, edited).unwrap();
    let g = lib.beatgrid(id).unwrap().unwrap();
    assert_eq!((g.grid, g.edited), (edited, true));
    // The edited tempo shows in the track list.
    let row = lib.track(id).unwrap();
    assert!((row.bpm.unwrap() - 174.05).abs() < 1e-3);

    // Re-analysis replaces the detected grid but leaves the edit alone.
    rec.grid = Some(Beatgrid::new(173.9, 0.1));
    lib.save_analysis(id, &rec, (0, 0)).unwrap();
    assert_eq!(lib.beatgrid(id).unwrap().unwrap().grid, edited);

    // Reset goes back to the (latest) detected grid.
    lib.reset_beatgrid(id).unwrap();
    let g = lib.beatgrid(id).unwrap().unwrap();
    assert_eq!((g.grid, g.edited), (Beatgrid::new(173.9, 0.1), false));
}

#[test]
fn removing_from_library_leaves_audio_files_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let music = tmp.path().join("music");
    let before = snapshot(&music);
    let all = ids(&lib);
    let set_a = lib.create_set("A").unwrap();
    let set_b = lib.create_set("B").unwrap();
    for set in [set_a, set_b] {
        lib.add_to_set(set, all[0]).unwrap();
        lib.add_to_set(set, all[1]).unwrap();
    }
    lib.save_analysis(all[0], &record(60.0, Some(120.0), None), (0, 0))
        .unwrap();
    assert_eq!(lib.sets_containing(all[0]).unwrap(), 2);

    lib.remove_track(all[0]).unwrap();

    // Gone from the library, its analysis and every set...
    assert_eq!(lib.track_count().unwrap(), 3);
    assert!(lib.track(all[0]).is_err());
    assert!(lib.analysis(all[0]).unwrap().is_none());
    for set in [set_a, set_b] {
        let order: Vec<_> = lib.set_tracks(set).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(order, vec![all[1]]);
    }
    // ...but every audio file is still there, byte for byte.
    assert_eq!(snapshot(&music), before);

    // Importing the folder again brings it back.
    let summary = lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(summary.added, 1);
    assert_eq!(lib.track_count().unwrap(), 4);
}

#[test]
fn removing_from_a_set_keeps_the_track_and_its_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let music = tmp.path().join("music");
    let before = snapshot(&music);
    let all = ids(&lib);
    let set = lib.create_set("Set").unwrap();
    lib.add_to_set(set, all[2]).unwrap();

    lib.remove_from_set(set, all[2]).unwrap();

    assert!(lib.set_tracks(set).unwrap().is_empty());
    assert_eq!(lib.track_count().unwrap(), 4);
    assert!(lib.track(all[2]).is_ok());
    assert_eq!(snapshot(&music), before);
}

#[test]
fn set_summary_totals_duration() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let all = ids(&lib);
    let set = lib.create_set("Totals").unwrap();
    for (&id, secs) in all.iter().zip([300.0, 390.0, 255.0]) {
        lib.save_analysis(id, &record(secs, None, None), (0, 0))
            .unwrap();
        lib.add_to_set(set, id).unwrap();
    }
    let s = lib.set_summary(set).unwrap();
    assert_eq!(s.track_count, 3);
    assert_eq!(s.total_secs, 945.0); // 15:45
}

#[test]
fn imports_dropped_files_and_folders() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("drop/a.mp3");
    let notes = tmp.path().join("drop/notes.txt");
    let folder = tmp.path().join("drop/album");
    copy("tone.mp3", &a);
    fs::write(&notes, "hi").unwrap();
    copy("tone.flac", &folder.join("1.flac"));
    copy("tone.wav", &folder.join("2.wav"));
    let mut lib = library(tmp.path());

    let s = lib
        .import_paths(
            &[a.clone(), notes.clone(), folder.clone(), a.clone()],
            |_| {},
        )
        .unwrap();
    assert_eq!((s.added, s.ignored), (3, 1));
    assert_eq!(s.tracks.len(), 3, "duplicate drop counted once");
    assert_eq!(lib.track(s.tracks[0]).unwrap().path, a);

    // Dropping again: nothing new, but the ids are still reported (for add-to-set).
    let again = lib.import_paths(&[folder], |_| {}).unwrap();
    assert_eq!((again.added, again.existing), (0, 2));
    assert_eq!(again.tracks, s.tracks[1..].to_vec());
}
