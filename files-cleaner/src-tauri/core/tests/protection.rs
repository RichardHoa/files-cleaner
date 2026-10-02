mod common;

use std::fs;
use std::path::Path;

use common::{config, empty_config, scan_root, write};
use files_cleaner_core::{scan, ScanError, ScanResult};

fn names(result: &ScanResult) -> Vec<String> {
    let mut names: Vec<String> = result.entries.iter().map(|e| e.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn skips_dev_folders_and_counts_them() {
    let dir = scan_root();
    write(dir.path(), "keep.txt", "k");
    write(dir.path(), "project/.git/HEAD", "ref");
    write(dir.path(), "project/node_modules/left-pad/index.js", "js");
    write(dir.path(), "project/.venv/bin/python", "py");
    write(dir.path(), "project/src/__pycache__/mod.pyc", "pyc");
    write(dir.path(), "rusty/Cargo.toml", "[package]");
    write(dir.path(), "rusty/target/debug/app", "bin");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["Cargo.toml", "keep.txt"]);
    assert_eq!(result.skipped, 5);
}

#[test]
fn a_target_folder_without_cargo_toml_is_ordinary() {
    let dir = scan_root();
    write(dir.path(), "marketing/target/audience.txt", "a");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["audience.txt"]);
    assert_eq!(result.skipped, 0);
}

#[test]
fn skips_finder_metadata_files() {
    let dir = scan_root();
    write(dir.path(), "photo.jpg", "jpg");
    write(dir.path(), ".DS_Store", "ds");
    write(dir.path(), "sub/.localized", "");
    write(dir.path(), "sub/Icon\r", "icon");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["photo.jpg"]);
    assert_eq!(result.skipped, 3);
}

#[test]
fn skips_app_libraries() {
    let dir = scan_root();
    write(dir.path(), "song.mp3", "mp3");
    write(dir.path(), "Pictures/Photos Library.photoslibrary/database/db", "db");
    write(dir.path(), "Music/Music/Media/track.m4a", "m4a");
    write(dir.path(), "Music/iTunes/iTunes Library.xml", "xml");
    write(dir.path(), "Music/Old.musiclibrary/x", "x");
    write(dir.path(), "Movies/TV.tvlibrary/x", "x");
    write(dir.path(), "Lightroom/Catalog.lrcat", "cat");
    write(dir.path(), "Lightroom/Catalog Previews.lrdata/p", "p");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["song.mp3"]);
    assert_eq!(result.skipped, 7);
}

#[test]
fn skips_the_holding_folder() {
    let dir = scan_root();
    write(dir.path(), "new.pdf", "pdf");
    write(dir.path(), "Files Cleaner Trash/session-1/old.pdf", "pdf");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["new.pdf"]);
    assert_eq!(result.skipped, 1);
}

#[test]
fn skips_the_users_library_folder() {
    let home = scan_root();
    write(home.path(), "Library/Preferences/app.plist", "plist");
    write(home.path(), "Documents/Library/book.epub", "epub");

    let result = scan(home.path(), &empty_config(home.path())).unwrap();

    assert_eq!(names(&result), ["book.epub"]);
    assert_eq!(result.skipped, 1);
}

#[test]
fn skips_never_touch_paths_from_the_config() {
    let home = scan_root();
    write(home.path(), "Downloads/Keep/precious.zip", "zip");
    write(home.path(), "Downloads/Other/precious.zip", "zip");
    write(home.path(), "Downloads/Tax/2025.pdf", "pdf");
    let tax = fs::canonicalize(home.path()).unwrap().join("Downloads/Tax");
    let text = format!(
        r#"{{ "neverTouch": ["~/Downloads/Keep", {}], "rules": [] }}"#,
        serde_json::to_string(&tax).unwrap()
    );

    let result = scan(&home.path().join("Downloads"), &config(&text, home.path())).unwrap();

    assert_eq!(result.entries.len(), 1);
    assert!(result.entries[0].path.ends_with("Other/precious.zip"));
    assert_eq!(result.skipped, 2);
}

fn refusal(root: &Path, home: &Path) -> String {
    match scan(root, &empty_config(home)) {
        Err(ScanError::ProtectedRoot { reason, .. }) => reason,
        other => panic!("expected {} to be refused, got {other:?}", root.display()),
    }
}

#[test]
fn refuses_system_folders_as_the_scan_root() {
    let home = scan_root();
    for root in ["/System", "/Library/Fonts", "/Applications", "/usr/bin", "/bin", "/sbin", "/private/etc", "/Volumes"] {
        refusal(Path::new(root), home.path());
    }
}

#[test]
fn refuses_a_scan_root_that_reaches_a_system_folder_through_a_symlink() {
    // `/tmp` is a symlink to `/private/tmp`.
    let home = scan_root();
    refusal(Path::new("/tmp"), home.path());
}

#[test]
fn refuses_a_volume_root_as_the_scan_root() {
    let home = scan_root();
    let volume = fs::read_dir("/Volumes").unwrap().next().unwrap().unwrap().path();
    refusal(&volume, home.path());
}

#[test]
fn refuses_protected_folders_and_anything_inside_them_as_the_scan_root() {
    let home = scan_root();
    let project = home.path().join("project");
    write(&project, "node_modules/pkg/lib/index.js", "js");
    write(&project, "Cargo.toml", "[package]");
    write(&project, "target/debug/app", "bin");
    write(home.path(), "Library/Caches/x", "x");
    write(home.path(), "Tool.app/Contents/Info.plist", "plist");

    let reason = refusal(&project.join("node_modules/pkg/lib"), home.path());
    assert!(reason.contains("node_modules"), "{reason}");
    refusal(&project.join("target/debug"), home.path());
    refusal(&home.path().join("Library/Caches"), home.path());
    refusal(&home.path().join("Tool.app/Contents"), home.path());
}

#[test]
fn refuses_a_never_touch_path_as_the_scan_root() {
    let home = scan_root();
    write(home.path(), "Downloads/Keep/a.txt", "a");
    let config = config(r#"{ "neverTouch": ["~/Downloads/Keep"], "rules": [] }"#, home.path());

    let refused = scan(&home.path().join("Downloads/Keep"), &config);

    assert!(matches!(refused, Err(ScanError::ProtectedRoot { .. })), "{refused:?}");
}

#[test]
fn refuses_the_startup_disk_root_as_the_scan_root() {
    let home = scan_root();
    refusal(Path::new("/"), home.path());
}

#[test]
fn a_folder_on_a_disk_under_volumes_can_be_scanned() {
    // The startup disk also appears under /Volumes; reach the test folder
    // through it, as if it were on an external disk.
    let Some(disk) = fs::read_dir("/Volumes")
        .unwrap()
        .map(|d| d.unwrap().path())
        .find(|d| fs::canonicalize(d).is_ok_and(|real| real == Path::new("/")))
    else {
        return;
    };
    let dir = scan_root();
    write(dir.path(), "a.txt", "a");
    let real = fs::canonicalize(dir.path()).unwrap();
    let via_volumes = disk.join(real.strip_prefix("/").unwrap());

    let result = scan(&via_volumes, &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["a.txt"]);
}
