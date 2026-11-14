//! Library tests covering the `vault`, `track-library` and `set-management` spec
//! scenarios.

use std::fs;
use std::path::{Path, PathBuf};

use djk_core::{Beatgrid, Key, Mode, TrackId};
use djk_library::{
    AnalysisRecord, FileRemoval, Library, LibraryError, SCHEMA_VERSION, SortKey, TrackQuery, Vault,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../djk-analysis/tests/fixtures")
        .join(name)
}

/// Copy a fixture byte for byte (the same content as every other copy of it).
fn copy(name: &str, to: &Path) {
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::copy(fixture(name), to).unwrap();
}

/// Copy a fixture with a few bytes appended so its content is unique to `to` (decoders
/// ignore trailing bytes), for tests that need several different files.
fn copy_unique(name: &str, to: &Path) {
    copy(name, to);
    let mut bytes = fs::read(to).unwrap();
    bytes.extend_from_slice(to.to_string_lossy().as_bytes());
    fs::write(to, bytes).unwrap();
}

/// Give a file an artist tag.
fn tag_artist(path: &Path, artist: &str) {
    use lofty::tag::{Accessor, Tag, TagExt, TagType};
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_artist(artist.to_owned());
    tag.save_to_path(path, lofty::config::WriteOptions::default())
        .unwrap();
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
        lufs: None,
    }
}

/// The test vault `<tmp>/Vault`, created on first use.
fn vault(tmp: &Path) -> Vault {
    let root = tmp.join("Vault");
    if Vault::is_vault(&root) {
        Vault::open(&root).unwrap()
    } else {
        Vault::create(tmp, "Vault").unwrap()
    }
}

fn library(tmp: &Path) -> Library {
    Library::open(&vault(tmp)).unwrap()
}

fn ids(lib: &Library) -> Vec<TrackId> {
    lib.tracks(&TrackQuery::default())
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect()
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

/// Every file under `dir`, relative to it, with `/` separators.
fn files_under(dir: &Path) -> Vec<String> {
    snapshot(dir)
        .into_iter()
        .map(|(p, _)| {
            p.strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let p = entry.unwrap().path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_dir(&p, &dest);
        } else {
            fs::copy(&p, &dest).unwrap();
        }
    }
}

fn db(tmp: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(tmp.join("Vault/.djkitty/library.db")).unwrap()
}

// ---- Vault and schema ----

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
fn leftover_partial_copies_are_removed_on_open() {
    let tmp = tempfile::tempdir().unwrap();
    copy("tone.mp3", &tmp.path().join("music/a.mp3"));
    {
        let mut lib = library(tmp.path());
        lib.import_folder(&tmp.path().join("music"), |_| {})
            .unwrap();
    }
    // A copy cut short by a power cut.
    let part = tmp.path().join("Vault/.djkitty/incoming/123-456-0.part");
    fs::write(&part, b"half a song").unwrap();
    let lib = library(tmp.path());
    assert!(!part.exists());
    assert_eq!(lib.track_count().unwrap(), 1);
    let id = ids(&lib)[0];
    assert!(lib.track(id).unwrap().path.is_file());
}

#[test]
fn a_moved_vault_keeps_working() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy_unique("tone.mp3", &music.join("a.mp3"));
    copy("tone.flac", &music.join("b.flac"));
    {
        let mut lib = library(tmp.path());
        lib.import_folder(&music, |_| {}).unwrap();
        let set = lib.create_set("Set").unwrap();
        for id in ids(&lib) {
            lib.add_to_set(set, id).unwrap();
        }
    }
    let moved = tmp.path().join("elsewhere/Gig Vault");
    copy_dir(&tmp.path().join("Vault"), &moved);
    fs::remove_dir_all(tmp.path().join("Vault")).unwrap();
    fs::remove_dir_all(&music).unwrap();

    let mut lib = Library::open_at(&moved).unwrap();
    assert_eq!(lib.vault().name(), "Gig Vault");
    let all = ids(&lib);
    assert_eq!(all.len(), 2);
    assert!(lib.tracks_needing_analysis().unwrap().len() == 2);
    for id in all {
        let path = lib.loadable_path(id).unwrap();
        assert!(path.starts_with(&moved), "{}", path.display());
        assert!(!lib.track(id).unwrap().missing);
    }
}

// ---- Import ----

#[test]
fn imports_supported_files_recursively_into_the_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    for i in 0..3 {
        copy_unique("tone.mp3", &music.join(format!("mp3/track{i}.mp3")));
    }
    copy_unique("tone.flac", &music.join("flac/one.flac"));
    copy_unique("tone.flac", &music.join("flac/deeper/two.flac"));
    let mut lib = library(tmp.path());
    let summary = lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(summary.added, 5);
    assert_eq!(lib.track_count().unwrap(), 5);
    let total: u64 = snapshot(&music).iter().map(|(_, b)| b.len() as u64).sum();
    assert_eq!(summary.bytes_copied, total);
    let catalog = tmp.path().join("Vault/Catalog");
    assert_eq!(
        files_under(&catalog),
        vec![
            "Unknown Artist/one.flac",
            "Unknown Artist/track0.mp3",
            "Unknown Artist/track1.mp3",
            "Unknown Artist/track2.mp3",
            "Unknown Artist/two.flac",
        ]
    );
    for t in lib.tracks(&TrackQuery::default()).unwrap() {
        assert!(t.path.starts_with(&catalog));
    }
}

#[test]
fn copies_are_byte_identical_to_the_originals() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("a.mp3"));
    copy("tone.flac", &music.join("b.flac"));
    copy("tone.wav", &music.join("c.wav"));
    let mut lib = library(tmp.path());
    let s = lib.import_folder(&music, |_| {}).unwrap();
    for id in s.tracks {
        let copy = lib.track(id).unwrap().path;
        let original = lib.source_path(id).unwrap().unwrap();
        assert_eq!(fs::read(&copy).unwrap(), fs::read(&original).unwrap());
        assert_ne!(copy, original);
    }
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
    assert_eq!(files_under(&tmp.path().join("Vault/Catalog")).len(), 1);
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
    let catalog = files_under(&tmp.path().join("Vault/Catalog"));
    assert!(
        !catalog.iter().any(|f| f.ends_with("bad.mp3")),
        "{catalog:?}"
    );
}

#[test]
fn reimport_adds_only_new_files() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy_unique("tone.mp3", &music.join("a.mp3"));
    copy_unique("tone.mp3", &music.join("b.mp3"));
    let mut lib = library(tmp.path());
    lib.import_folder(&music, |_| {}).unwrap();
    let before = lib.tracks(&TrackQuery::default()).unwrap();

    copy("tone.flac", &music.join("c.flac"));
    let summary = lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(summary.added, 1);
    assert_eq!(summary.existing, 2);
    assert_eq!(
        summary.bytes_copied,
        fs::metadata(music.join("c.flac")).unwrap().len()
    );
    let after = lib.tracks(&TrackQuery::default()).unwrap();
    assert_eq!(after.len(), 3);
    for t in &before {
        assert!(after.contains(t), "existing track unchanged: {t:?}");
    }
    assert_eq!(files_under(&tmp.path().join("Vault/Catalog")).len(), 3);
}

#[test]
fn the_same_file_from_two_places_is_one_track_and_one_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let downloads = tmp.path().join("Downloads/Glue.mp3");
    let usb = tmp.path().join("USB/Promos/Glue.mp3");
    copy("tone.mp3", &downloads);
    copy("tone.mp3", &usb);
    let mut lib = library(tmp.path());
    let first = lib.import_paths(&[downloads], |_| {}).unwrap();
    let second = lib.import_paths(&[usb], |_| {}).unwrap();
    assert_eq!((first.added, second.added, second.existing), (1, 0, 1));
    assert_eq!(second.tracks, first.tracks);
    assert_eq!(second.bytes_copied, 0);
    assert_eq!(lib.track_count().unwrap(), 1);
    assert_eq!(files_under(&tmp.path().join("Vault/Catalog")).len(), 1);
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

#[test]
fn catalog_layout_uses_the_artist_and_the_original_name() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tagged.mp3", &music.join("02 Glue.mp3"));
    copy("tone.mp3", &music.join("my_mix_track.mp3"));
    let mut lib = library(tmp.path());
    lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(
        files_under(&tmp.path().join("Vault/Catalog")),
        vec![
            "Unknown Artist/my_mix_track.mp3",
            "Ólafur Example/02 Glue.mp3"
        ]
    );
}

#[test]
fn a_different_file_with_the_same_name_gets_a_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let first = tmp.path().join("one/01 Intro.mp3");
    let second = tmp.path().join("two/01 Intro.mp3");
    let third = tmp.path().join("three/01 INTRO.mp3");
    copy_unique("tone.mp3", &first);
    copy_unique("tone.mp3", &second);
    copy_unique("tone.mp3", &third);
    let mut lib = library(tmp.path());
    lib.import_paths(std::slice::from_ref(&first), |_| {})
        .unwrap();
    let before = fs::read(tmp.path().join("Vault/Catalog/Unknown Artist/01 Intro.mp3")).unwrap();
    lib.import_paths(&[second, third], |_| {}).unwrap();
    assert_eq!(
        files_under(&tmp.path().join("Vault/Catalog")),
        vec![
            "Unknown Artist/01 INTRO (3).mp3",
            "Unknown Artist/01 Intro (2).mp3",
            "Unknown Artist/01 Intro.mp3",
        ]
    );
    assert_eq!(
        fs::read(tmp.path().join("Vault/Catalog/Unknown Artist/01 Intro.mp3")).unwrap(),
        before,
        "existing file unchanged"
    );
}

#[test]
fn artist_names_windows_refuses_are_cleaned_for_the_folder_only() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("music/live.mp3");
    copy("tone.mp3", &file);
    tag_artist(&file, "AC/DC: Live");
    let mut lib = library(tmp.path());
    let s = lib.import_paths(&[file], |_| {}).unwrap();
    let row = lib.track(s.tracks[0]).unwrap();
    assert_eq!(row.artist, "AC/DC: Live");
    assert_eq!(
        files_under(&tmp.path().join("Vault/Catalog")),
        vec!["AC_DC_ Live/live.mp3"]
    );
}

#[test]
fn a_file_that_changes_while_importing_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("a.mp3"));
    copy("tone.flac", &music.join("b.flac"));
    let mut lib = library(tmp.path());
    let s = lib
        .import_paths_with_hook(
            std::slice::from_ref(&music),
            |_| {},
            |p| {
                if p.ends_with("a.mp3") {
                    let mut bytes = fs::read(p).unwrap();
                    bytes.extend_from_slice(b"still downloading");
                    fs::write(p, bytes).unwrap();
                }
            },
        )
        .unwrap();
    assert_eq!((s.added, s.skipped), (1, 1));
    assert_eq!(
        files_under(&tmp.path().join("Vault/Catalog")),
        vec!["Unknown Artist/b.flac"]
    );
    assert!(files_under(&tmp.path().join("Vault/.djkitty/incoming")).is_empty());
}

#[test]
fn a_failed_insert_leaves_no_catalog_file() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("a.mp3"));
    let mut lib = library(tmp.path());
    db(tmp.path())
        .execute_batch(
            "CREATE TRIGGER refuse BEFORE INSERT ON tracks BEGIN SELECT RAISE(ABORT, 'no'); END;",
        )
        .unwrap();
    assert!(lib.import_folder(&music, |_| {}).is_err());
    assert_eq!(lib.track_count().unwrap(), 0);
    assert!(files_under(&tmp.path().join("Vault/Catalog")).is_empty());
    assert!(files_under(&tmp.path().join("Vault/.djkitty/incoming")).is_empty());
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
    assert_eq!(lib.source_path(s.tracks[0]).unwrap(), Some(a));

    // Dropping again: nothing new, but the ids are still reported (for add-to-set).
    let again = lib.import_paths(&[folder], |_| {}).unwrap();
    assert_eq!((again.added, again.existing), (0, 2));
    assert_eq!(again.tracks, s.tracks[1..].to_vec());
}

// ---- Browsing ----

fn library_with_titles(tmp: &Path) -> Library {
    let music = tmp.join("music");
    copy("tagged.mp3", &music.join("tagged.mp3"));
    for name in ["alpha", "Bravo", "charlie"] {
        copy_unique("tone.mp3", &music.join(format!("{name}.mp3")));
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
    // Loudness: alpha is louder than Bravo, so it comes last; tracks without a reading
    // always sort last.
    let mut louder = record(300.0, None, None);
    louder.lufs = Some(-7.5);
    let mut quieter = record(200.0, None, None);
    quieter.lufs = Some(-11.0);
    lib.save_analysis(rows[0].id, &louder, (0, 0)).unwrap();
    lib.save_analysis(rows[1].id, &quieter, (0, 0)).unwrap();
    assert_eq!(&titles(&lib, SortKey::Lufs)[..2], &["Bravo", "alpha"]);
    let alpha = lib.tracks(&TrackQuery::default()).unwrap();
    assert_eq!(
        alpha.iter().find(|t| t.title == "alpha").unwrap().lufs,
        Some(-7.5)
    );
    assert_eq!(lib.analysis(rows[1].id).unwrap().unwrap().lufs, Some(-11.0));
}

// ---- Missing files and analysis ----

#[test]
fn deleted_file_is_flagged_missing_and_cannot_load() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("gone.mp3"));
    let mut lib = library(tmp.path());
    lib.import_folder(&music, |_| {}).unwrap();
    let id = ids(&lib)[0];
    let set = lib.create_set("Set").unwrap();
    lib.add_to_set(set, id).unwrap();
    let copy = lib.loadable_path(id).unwrap();

    fs::remove_file(copy).unwrap();
    let changed = lib.refresh_missing(&[id]).unwrap();
    assert_eq!(changed, vec![id]);
    assert!(lib.track(id).unwrap().missing);
    assert!(matches!(
        lib.loadable_path(id),
        Err(LibraryError::Missing(_))
    ));
    // Still in the set, in its place, flagged.
    let in_set = lib.set_tracks(set).unwrap();
    assert_eq!(in_set.len(), 1);
    assert!(in_set[0].missing);
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

    // Overwrite the vault copy with a different file.
    copy("tone.flac", &path);
    assert_eq!(lib.tracks_needing_analysis().unwrap().len(), 1);
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
    db(tmp.path())
        .execute("UPDATE analysis SET version = 1", [])
        .unwrap();
    let mut lib = library(tmp.path());
    assert!(
        lib.tracks_needing_analysis()
            .unwrap()
            .iter()
            .any(|(t, _)| *t == id)
    );
}

#[test]
fn a_file_that_comes_back_is_no_longer_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    copy("tone.mp3", &music.join("a.mp3"));
    let mut lib = library(tmp.path());
    lib.import_folder(&music, |_| {}).unwrap();
    let (id, path) = lib.tracks_needing_analysis().unwrap()[0].clone();
    lib.save_analysis(id, &record(1.0, None, None), Library::stamp(&path).unwrap())
        .unwrap();

    // The file goes away...
    let away = tmp.path().join("away.mp3");
    fs::rename(&path, &away).unwrap();
    assert!(lib.tracks_needing_analysis().unwrap().is_empty());
    assert!(lib.track(id).unwrap().missing);
    // ...and comes back unchanged.
    fs::rename(&away, &path).unwrap();
    assert!(lib.tracks_needing_analysis().unwrap().is_empty());
    assert!(!lib.track(id).unwrap().missing);
}

// ---- Beatgrids and hot cues ----

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
fn hot_cues_round_trip_clear_and_go_with_the_track() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let all = ids(&lib);
    let id = all[0];
    assert_eq!(lib.hot_cues(id).unwrap(), [None; 8], "fresh track has none");

    let mut cues = [None; 8];
    cues[0] = Some(31.0);
    cues[4] = Some(62.35);
    lib.set_hot_cues(id, &cues).unwrap();
    assert_eq!(lib.hot_cues(id).unwrap(), cues);
    assert_eq!(
        lib.hot_cues(all[1]).unwrap(),
        [None; 8],
        "other tracks untouched"
    );

    // Clearing a slot removes it; the rest stay.
    cues[0] = None;
    lib.set_hot_cues(id, &cues).unwrap();
    assert_eq!(lib.hot_cues(id).unwrap(), cues);

    // Survives reopening the library.
    drop(lib);
    let mut lib = library(tmp.path());
    assert_eq!(lib.hot_cues(id).unwrap()[4], Some(62.35));

    // Removing the track removes its cues.
    lib.remove_track(id).unwrap();
    assert_eq!(lib.hot_cues(id).unwrap(), [None; 8]);
    assert!(lib.set_hot_cues(id, &cues).is_err(), "no such track");
}

#[test]
fn editing_a_track_never_changes_its_vault_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let catalog = tmp.path().join("Vault/Catalog");
    let before = snapshot(&catalog);
    for id in ids(&lib) {
        let mut cues = [None; 8];
        cues[1] = Some(4.0);
        lib.set_hot_cues(id, &cues).unwrap();
        lib.set_beatgrid(id, Beatgrid::with_phase(126.0, 0.2, 1))
            .unwrap();
        lib.save_analysis(id, &record(60.0, Some(126.0), None), (0, 0))
            .unwrap();
    }
    assert_eq!(snapshot(&catalog), before);
}

// ---- Removal ----

#[test]
fn removing_a_track_deletes_its_vault_copy_only() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let music = tmp.path().join("music");
    let originals = snapshot(&music);
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
    let copy = lib.track(all[0]).unwrap().path;

    assert_eq!(lib.remove_track(all[0]).unwrap(), FileRemoval::Deleted);

    // Gone from the library, its analysis, every set and the vault...
    assert_eq!(lib.track_count().unwrap(), 3);
    assert!(lib.track(all[0]).is_err());
    assert!(lib.analysis(all[0]).unwrap().is_none());
    for set in [set_a, set_b] {
        let order: Vec<_> = lib.set_tracks(set).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(order, vec![all[1]]);
    }
    assert!(!copy.exists());
    // ...but every original file is still there, byte for byte.
    assert_eq!(snapshot(&music), originals);

    // Importing the folder again brings it back as a new track, with nothing carried
    // over.
    let summary = lib.import_folder(&music, |_| {}).unwrap();
    assert_eq!(summary.added, 1);
    assert_eq!(lib.track_count().unwrap(), 4);
    let back = *summary.tracks.iter().max().unwrap();
    assert_ne!(back, all[0]);
    assert_eq!(lib.hot_cues(back).unwrap(), [None; 8]);
    assert_eq!(lib.sets_containing(back).unwrap(), 0);
}

#[test]
fn an_emptied_artist_folder_is_removed() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let tagged = lib
        .tracks(&TrackQuery::default())
        .unwrap()
        .into_iter()
        .find(|t| t.artist == "Ólafur Example")
        .unwrap();
    lib.remove_track(tagged.id).unwrap();
    assert!(!tmp.path().join("Vault/Catalog/Ólafur Example").exists());
    assert!(tmp.path().join("Vault/Catalog/Unknown Artist").is_dir());
}

#[test]
fn originals_are_never_changed() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let music = tmp.path().join("music");
    let originals = snapshot(&music);
    lib.import_folder(&music, |_| {}).unwrap();
    for id in ids(&lib) {
        lib.remove_track(id).unwrap();
    }
    assert_eq!(lib.track_count().unwrap(), 0);
    assert!(files_under(&tmp.path().join("Vault/Catalog")).is_empty());
    assert_eq!(snapshot(&music), originals);
}

#[test]
fn removal_never_deletes_outside_the_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let outside_dir = tmp.path().join("precious");
    let outside = outside_dir.join("keep.mp3");
    copy("tone.mp3", &outside);
    let mut tampered: Vec<String> = vec![
        // An absolute path.
        outside.to_string_lossy().into_owned(),
        // Climbing out of the Catalog.
        "Catalog/../../precious/keep.mp3".to_owned(),
        // Climbing out of the vault without mentioning the Catalog.
        "../precious/keep.mp3".to_owned(),
    ];
    // Through a link inside the Catalog that points outside it.
    let link = tmp.path().join("Vault").join("Catalog").join("Link");
    #[cfg(windows)]
    let linked = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&outside_dir)
        .output()
        .is_ok_and(|o| o.status.success());
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside_dir, &link).is_ok();
    // Junctions need no special rights on Windows, so that case must always run there.
    #[cfg(windows)]
    assert!(linked, "could not create a junction for the test");
    if linked {
        tampered.push("Catalog/Link/keep.mp3".to_owned());
    }
    let all = ids(&lib);
    assert!(all.len() >= tampered.len());
    for (id, path) in all.iter().zip(&tampered) {
        db(tmp.path())
            .execute(
                "UPDATE tracks SET path = ?2 WHERE id = ?1",
                rusqlite::params![id.0, path],
            )
            .unwrap();
        assert_eq!(
            lib.remove_track(*id).unwrap(),
            FileRemoval::Refused,
            "{path}"
        );
        assert!(lib.track(*id).is_err(), "still removed from the library");
        assert!(outside.is_file(), "{path} deleted the outside file");
    }
}

#[test]
fn removing_from_a_set_keeps_the_track_and_its_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let catalog = tmp.path().join("Vault/Catalog");
    let before = snapshot(&catalog);
    let all = ids(&lib);
    let set = lib.create_set("Set").unwrap();
    lib.add_to_set(set, all[2]).unwrap();

    lib.remove_from_set(set, all[2]).unwrap();

    assert!(lib.set_tracks(set).unwrap().is_empty());
    assert_eq!(lib.track_count().unwrap(), 4);
    assert!(lib.track(all[2]).is_ok());
    assert_eq!(snapshot(&catalog), before);
}

#[test]
fn a_removed_tracks_id_is_never_reused() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let newest = *ids(&lib).iter().max().unwrap();
    lib.remove_track(newest).unwrap();
    let music = tmp.path().join("more");
    copy("tone.flac", &music.join("new.flac"));
    let s = lib.import_folder(&music, |_| {}).unwrap();
    assert!(s.tracks[0] > newest, "{:?} reused {newest:?}", s.tracks[0]);
}

// ---- Locate file ----

#[test]
fn locating_the_original_restores_a_missing_track() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let id = ids(&lib)[0];
    let mut cues = [None; 8];
    cues[2] = Some(9.5);
    lib.set_hot_cues(id, &cues).unwrap();
    let set = lib.create_set("Set").unwrap();
    lib.add_to_set(set, id).unwrap();
    let copy = lib.track(id).unwrap().path;
    let original = lib.source_path(id).unwrap().unwrap();
    fs::remove_file(&copy).unwrap();
    lib.refresh_missing(&[id]).unwrap();
    assert!(lib.track(id).unwrap().missing);

    lib.relink(id, &original).unwrap();
    assert!(!lib.track(id).unwrap().missing);
    assert_eq!(lib.loadable_path(id).unwrap(), copy);
    assert_eq!(fs::read(&copy).unwrap(), fs::read(&original).unwrap());
    assert_eq!(lib.hot_cues(id).unwrap(), cues);
    assert_eq!(lib.set_tracks(set).unwrap()[0].id, id);
}

#[test]
fn locating_a_different_file_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let id = ids(&lib)[0];
    let copy = lib.track(id).unwrap().path;
    fs::remove_file(&copy).unwrap();
    lib.refresh_missing(&[id]).unwrap();
    let other = tmp.path().join("other.flac");
    fs::copy(fixture("tone.flac"), &other).unwrap();

    assert!(matches!(
        lib.relink(id, &other),
        Err(LibraryError::NotSameFile)
    ));
    assert!(!copy.exists());
    assert!(lib.track(id).unwrap().missing);
    assert!(files_under(&tmp.path().join("Vault/.djkitty/incoming")).is_empty());
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

// ---- Set playlist files ----

fn sets_dir(tmp: &Path) -> PathBuf {
    tmp.join("Vault/Sets")
}

/// The paths listed in a playlist file, each resolved against the `Sets` folder.
fn playlist_entries(file: &Path) -> Vec<PathBuf> {
    let text = fs::read_to_string(file).unwrap();
    assert!(text.starts_with("#EXTM3U\n"), "{text}");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| file.parent().unwrap().join(l))
        .collect()
}

fn same_file(a: &Path, b: &Path) -> bool {
    fs::canonicalize(a).unwrap() == fs::canonicalize(b).unwrap()
}

#[test]
fn a_set_is_mirrored_as_a_playlist_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let all = ids(&lib);
    let set = lib.create_set("Friday warmup").unwrap();
    let file = sets_dir(tmp.path()).join("Friday warmup.m3u8");
    assert!(
        playlist_entries(&file).is_empty(),
        "empty set, empty playlist"
    );
    lib.save_analysis(all[0], &record(372.4, None, None), (0, 0))
        .unwrap();
    lib.add_to_set(set, all[0]).unwrap();
    lib.add_to_set(set, all[1]).unwrap();

    let entries = playlist_entries(&file);
    assert_eq!(entries.len(), 2);
    for (entry, id) in entries.iter().zip(&all) {
        assert!(entry.is_file(), "{} resolves", entry.display());
        assert!(same_file(entry, &lib.track(*id).unwrap().path));
        assert!(entry.to_string_lossy().contains("../Catalog/"));
    }
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains("#EXTINF:372,alpha\n"), "{text}");
    assert_eq!(
        lib.set_file_name(set).unwrap().as_deref(),
        Some("Friday warmup.m3u8")
    );
}

#[test]
fn reordering_and_removing_update_the_playlist_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let all = ids(&lib);
    let set = lib.create_set("Order").unwrap();
    let other = lib.create_set("Other").unwrap();
    for &id in &all[..3] {
        lib.add_to_set(set, id).unwrap();
    }
    lib.add_to_set(other, all[2]).unwrap();
    let file = sets_dir(tmp.path()).join("Order.m3u8");
    let path_of = |lib: &Library, id: TrackId| lib.track(id).unwrap().path;

    lib.move_in_set(set, 2, 0).unwrap();
    let first = playlist_entries(&file)[0].clone();
    assert!(same_file(&first, &path_of(&lib, all[2])));

    lib.remove_from_set(set, all[0]).unwrap();
    assert_eq!(playlist_entries(&file).len(), 2);

    // A track in two sets removed from the library leaves both playlists.
    lib.remove_track(all[2]).unwrap();
    assert_eq!(playlist_entries(&file).len(), 1);
    assert!(playlist_entries(&sets_dir(tmp.path()).join("Other.m3u8")).is_empty());
}

#[test]
fn renaming_and_deleting_a_set_follow_its_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let all = ids(&lib);
    let catalog_before = snapshot(&tmp.path().join("Vault/Catalog"));
    let set = lib.create_set("Friday warmup").unwrap();
    for &id in &all {
        lib.add_to_set(set, id).unwrap();
    }
    lib.rename_set(set, "Friday peak").unwrap();
    assert_eq!(files_under(&sets_dir(tmp.path())), vec!["Friday peak.m3u8"]);

    lib.delete_set(set).unwrap();
    assert!(files_under(&sets_dir(tmp.path())).is_empty());
    assert_eq!(lib.track_count().unwrap(), 4);
    assert_eq!(snapshot(&tmp.path().join("Vault/Catalog")), catalog_before);
}

#[test]
fn set_file_names_are_cleaned_and_kept_apart() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library(tmp.path());
    let late = lib.create_set("Warmup 22:00").unwrap();
    assert_eq!(lib.set_summary(late).unwrap().name, "Warmup 22:00");
    let a = lib.create_set("A").unwrap();
    let a2 = lib.create_set("a").unwrap();
    assert_eq!(
        files_under(&sets_dir(tmp.path())),
        vec!["A.m3u8", "Warmup 22_00.m3u8", "a (2).m3u8"]
    );
    // Deleting the first frees nothing it doesn't own.
    lib.delete_set(a).unwrap();
    assert_eq!(
        lib.set_file_name(a2).unwrap().as_deref(),
        Some("a (2).m3u8")
    );
    assert!(sets_dir(tmp.path()).join("a (2).m3u8").is_file());
}

#[test]
fn playlist_files_are_rewritten_from_the_library_on_open() {
    let tmp = tempfile::tempdir().unwrap();
    let file = sets_dir(tmp.path()).join("Set.m3u8");
    {
        let mut lib = library_with_titles(tmp.path());
        let set = lib.create_set("Set").unwrap();
        for id in ids(&lib) {
            lib.add_to_set(set, id).unwrap();
        }
    }
    let full = fs::read_to_string(&file).unwrap();
    // Delete a line by hand.
    let edited: Vec<&str> = full.lines().take(3).collect();
    fs::write(&file, edited.join("\n")).unwrap();
    let lib = library(tmp.path());
    assert_eq!(fs::read_to_string(&file).unwrap(), full);
    assert_eq!(lib.sets().unwrap()[0].track_count, 4);
}

#[test]
fn foreign_playlist_files_are_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let foreign = sets_dir(tmp.path()).join("Old crate.m3u8");
    {
        let _lib = library(tmp.path());
    }
    fs::write(&foreign, "#EXTM3U\nD:/Music/x.mp3\n").unwrap();
    let mut lib = library(tmp.path());
    assert!(lib.sets().unwrap().is_empty());
    // A set with the same name gets its own file.
    let set = lib.create_set("Old crate").unwrap();
    assert_eq!(
        lib.set_file_name(set).unwrap().as_deref(),
        Some("Old crate (2).m3u8")
    );
    lib.delete_set(set).unwrap();
    drop(lib);
    let _lib = library(tmp.path());
    assert_eq!(
        fs::read_to_string(&foreign).unwrap(),
        "#EXTM3U\nD:/Music/x.mp3\n"
    );
}

#[test]
fn a_playlist_file_that_cannot_be_written_is_only_a_warning() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = library_with_titles(tmp.path());
    let id = ids(&lib)[0];
    // Something replaced the Sets folder with a file.
    let sets = sets_dir(tmp.path());
    fs::remove_dir_all(&sets).unwrap();
    fs::write(&sets, b"not a folder").unwrap();

    let set = lib.create_set("Blocked").unwrap();
    assert!(lib.add_to_set(set, id).unwrap());
    assert_eq!(lib.set_tracks(set).unwrap().len(), 1, "the change is kept");
    let warnings = lib.take_warnings();
    assert!(!warnings.is_empty());
    assert!(warnings[0].contains("Blocked"), "{warnings:?}");
    assert!(lib.take_warnings().is_empty(), "taken once");
}
