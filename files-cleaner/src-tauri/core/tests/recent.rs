use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use files_cleaner_core::{record_recent_scan_root, recent_scan_roots, remove_recent_scan_root};
use tempfile::TempDir;

/// An app data folder with a folder per name in it, standing in for Scan
/// Roots; returns it, the store's path and each folder's path.
fn setup(names: &[&str]) -> (TempDir, PathBuf, Vec<PathBuf>) {
    let dir = TempDir::new().unwrap();
    let folders = names
        .iter()
        .map(|name| {
            let path = dir.path().join(name);
            fs::create_dir(&path).unwrap();
            path
        })
        .collect();
    let store = dir.path().join("recent-scan-roots.json");
    (dir, store, folders)
}

fn paths(store: &Path) -> Vec<PathBuf> {
    recent_scan_roots(store).into_iter().map(|recent| recent.path).collect()
}

#[test]
fn the_latest_scan_root_comes_first() {
    let (_dir, store, folders) = setup(&["a", "b"]);

    record_recent_scan_root(&store, &folders[0]).unwrap();
    record_recent_scan_root(&store, &folders[1]).unwrap();

    assert_eq!(paths(&store), [folders[1].clone(), folders[0].clone()]);
}

#[test]
fn scanning_a_folder_again_moves_it_to_the_front_without_a_repeat() {
    let (_dir, store, folders) = setup(&["a", "b", "c"]);

    for folder in [&folders[0], &folders[1], &folders[2], &folders[0]] {
        record_recent_scan_root(&store, folder).unwrap();
    }

    assert_eq!(paths(&store), [folders[0].clone(), folders[2].clone(), folders[1].clone()]);
}

#[test]
fn only_the_newest_three_are_kept() {
    let (_dir, store, folders) = setup(&["a", "b", "c", "d"]);

    for folder in &folders {
        record_recent_scan_root(&store, folder).unwrap();
    }

    assert_eq!(paths(&store), [folders[3].clone(), folders[2].clone(), folders[1].clone()]);
}

#[test]
fn each_one_says_when_it_was_last_scanned_and_whether_it_still_exists() {
    let (_dir, store, folders) = setup(&["a", "b"]);
    let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
    record_recent_scan_root(&store, &folders[0]).unwrap();
    record_recent_scan_root(&store, &folders[1]).unwrap();
    let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
    fs::remove_dir(&folders[0]).unwrap();

    let recent = recent_scan_roots(&store);

    assert!(recent[0].exists);
    assert!(!recent[1].exists, "a deleted folder is listed as missing");
    for one in &recent {
        assert!((before..=after).contains(&one.last_scanned), "{} not in {before}..={after}", one.last_scanned);
    }
}

#[test]
fn a_recent_scan_root_can_be_removed() {
    let (_dir, store, folders) = setup(&["a", "b"]);
    record_recent_scan_root(&store, &folders[0]).unwrap();
    record_recent_scan_root(&store, &folders[1]).unwrap();

    remove_recent_scan_root(&store, &folders[1]).unwrap();

    assert_eq!(paths(&store), [folders[0].clone()]);
}

#[test]
fn a_missing_store_is_an_empty_list() {
    let (_dir, store, _) = setup(&[]);

    assert!(recent_scan_roots(&store).is_empty());
    remove_recent_scan_root(&store, Path::new("/nowhere")).unwrap();
}

#[test]
fn a_corrupt_store_is_an_empty_list_and_is_replaced_on_the_next_scan() {
    let (_dir, store, folders) = setup(&["a"]);
    fs::write(&store, "{ not json").unwrap();

    assert!(recent_scan_roots(&store).is_empty());

    record_recent_scan_root(&store, &folders[0]).unwrap();
    assert_eq!(paths(&store), [folders[0].clone()]);
}

#[test]
fn the_store_is_created_with_its_folder_and_survives_reopening() {
    let dir = TempDir::new().unwrap();
    let store = dir.path().join("app data").join("recent-scan-roots.json");
    let folder = dir.path().to_path_buf();

    record_recent_scan_root(&store, &folder).unwrap();

    // Nothing is cached: a fresh read, as after a restart, finds it.
    assert!(store.exists());
    assert_eq!(paths(&store), [folder]);
}
