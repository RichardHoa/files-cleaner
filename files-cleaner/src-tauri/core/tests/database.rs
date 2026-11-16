use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime};

use files_cleaner_core::{
    delete_database, is_damaged, open_database, write_atomically, Durability,
};
use rusqlite::Connection;

const V1: &str = "CREATE TABLE things (name TEXT NOT NULL) STRICT;";
const V2: &str = "ALTER TABLE things ADD COLUMN size INTEGER;";

fn user_version(db: &Connection) -> i64 {
    db.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

#[test]
fn a_new_database_gets_every_migration_and_reopening_runs_none_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.sqlite");

    let db = open_database(&path, Durability::Full, &[V1, V2]).unwrap();
    db.execute("INSERT INTO things (name, size) VALUES ('x', 1)", [])
        .unwrap();
    drop(db);
    // V1 again would fail: the table is already there.
    let db = open_database(&path, Durability::Full, &[V1, V2]).unwrap();

    assert_eq!(user_version(&db), 2);
    let journal_mode: String = db
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode, "wal");
    let count: i64 = db
        .query_row("SELECT count(*) FROM things", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn an_older_database_gets_only_the_migrations_it_lacks_and_keeps_its_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.sqlite");
    let db = open_database(&path, Durability::Normal, &[V1]).unwrap();
    db.execute("INSERT INTO things (name) VALUES ('kept')", [])
        .unwrap();
    drop(db);

    let db = open_database(&path, Durability::Normal, &[V1, V2]).unwrap();

    assert_eq!(user_version(&db), 2);
    let (name, size): (String, Option<i64>) = db
        .query_row("SELECT name, size FROM things", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!((name.as_str(), size), ("kept", None));
}

#[test]
fn a_database_from_a_newer_version_is_opened_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.sqlite");
    drop(open_database(&path, Durability::Normal, &[V1, V2]).unwrap());

    let db = open_database(&path, Durability::Normal, &[V1]).unwrap();

    assert_eq!(user_version(&db), 2);
}

#[test]
fn a_migration_that_fails_leaves_the_database_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.sqlite");
    drop(open_database(&path, Durability::Normal, &[V1]).unwrap());

    let broken = "ALTER TABLE things ADD COLUMN size INTEGER; NOT SQL;";
    assert!(open_database(&path, Durability::Normal, &[V1, broken]).is_err());

    let db = open_database(&path, Durability::Normal, &[V1, V2]).unwrap();
    assert_eq!(user_version(&db), 2);
}

#[test]
fn many_opening_a_new_database_at_once_migrate_it_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.sqlite");

    let opened: Vec<_> = (0..8)
        .map(|_| {
            let path = path.clone();
            thread::spawn(move || open_database(&path, Durability::Full, &[V1, V2]).map(|_| ()))
        })
        .collect();

    for opened in opened {
        opened.join().unwrap().unwrap();
    }
    let db = open_database(&path, Durability::Full, &[V1, V2]).unwrap();
    assert_eq!(user_version(&db), 2);
}

#[test]
fn a_file_that_isnt_a_database_is_damaged_and_deleting_it_takes_its_side_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.sqlite");
    fs::write(&path, "not a database, but long enough to have a header").unwrap();
    fs::write(dir.path().join("a.sqlite-wal"), "left over").unwrap();
    fs::write(dir.path().join("a.sqlite-shm"), "left over").unwrap();

    let e = open_database(&path, Durability::Normal, &[V1]).unwrap_err();
    assert!(is_damaged(&e));
    delete_database(&path).unwrap();

    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    open_database(&path, Durability::Normal, &[V1]).unwrap();
}

#[test]
fn deleting_a_database_that_isnt_there_is_fine() {
    let dir = tempfile::tempdir().unwrap();

    delete_database(&dir.path().join("a.sqlite")).unwrap();
}

/// Every name in `folder`.
fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|child| child.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn writing_atomically_replaces_the_file_and_leaves_nothing_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    fs::write(&path, "old, and longer than the new contents").unwrap();

    write_atomically(&path, b"new").unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(names(dir.path()), ["config.json"]);
}

#[test]
fn writing_atomically_clears_only_temporary_files_a_crash_left_long_ago() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let old = dir.path().join("config.json.1-0.tmp");
    let recent = dir.path().join("config.json.1-1.tmp");
    let other = dir.path().join("recent.json.1-0.tmp");
    for left in [&old, &recent, &other] {
        fs::write(left, "half").unwrap();
    }
    let hours_ago = SystemTime::now() - Duration::from_secs(2 * 60 * 60);
    for left in [&old, &other] {
        fs::File::options()
            .write(true)
            .open(left)
            .unwrap()
            .set_modified(hours_ago)
            .unwrap();
    }

    write_atomically(&path, b"new").unwrap();

    assert!(!old.exists());
    assert!(recent.exists());
    assert!(other.exists());
}

#[test]
fn writes_at_once_each_leave_a_whole_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let contents: Vec<String> = (0..8).map(|i| i.to_string().repeat(10_000)).collect();

    let writes: Vec<_> = contents
        .iter()
        .cloned()
        .map(|text| {
            let path = path.clone();
            thread::spawn(move || write_atomically(&path, text.as_bytes()))
        })
        .collect();
    for write in writes {
        write.join().unwrap().unwrap();
    }

    assert!(contents.contains(&fs::read_to_string(&path).unwrap()));
    assert_eq!(names(dir.path()), ["config.json"]);
}

#[test]
fn writing_atomically_into_a_missing_folder_fails_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();

    assert!(write_atomically(&dir.path().join("missing/config.json"), b"new").is_err());

    assert!(names(dir.path()).is_empty());
}
