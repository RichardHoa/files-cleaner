mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{empty_config, scan_root, write};
use files_cleaner_core::{scan, scan_with_progress, ScanResult};

/// Paths of the scanned entries relative to the Scan Root, sorted. Also
/// checks each entry's `relative_path` names the same path.
fn relative_paths(result: &ScanResult, root: &Path) -> Vec<String> {
    let root = fs::canonicalize(root).unwrap();
    let mut paths: Vec<String> = result
        .entries
        .iter()
        .map(|e| {
            let relative = e.path.strip_prefix(&root).unwrap();
            assert_eq!(relative, e.relative_path.iter().collect::<PathBuf>(), "{}", e.path.display());
            relative.to_string_lossy().into_owned()
        })
        .collect();
    paths.sort();
    paths
}

#[test]
fn lists_every_file_in_every_subfolder() {
    let dir = scan_root();
    write(dir.path(), "report.pdf", "pdf");
    write(dir.path(), "a/notes.txt", "notes");
    write(dir.path(), "a/b/c/deep.pptx", "slides");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(
        relative_paths(&result, dir.path()),
        ["a/b/c/deep.pptx", "a/notes.txt", "report.pdf"]
    );
}

#[test]
fn reports_name_extension_size_and_dates() {
    let dir = scan_root();
    let path = write(dir.path(), "docs/Lecture 3.PPTX", "twelve bytes");
    // 2024-03-05T10:00:00Z
    let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_709_632_800);
    fs::File::options().write(true).open(&path).unwrap().set_modified(modified).unwrap();

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    let lecture = result.entries.iter().find(|e| e.name == "Lecture 3.PPTX").unwrap();
    assert_eq!(lecture.extension, "PPTX");
    assert_eq!(lecture.size, 12);
    assert_eq!(lecture.modified, 1_709_632_800_000);
    assert!(lecture.created.is_some());
    assert!(!lecture.is_package);
}

#[test]
fn leaves_out_files_with_no_extension_and_counts_them() {
    let dir = scan_root();
    write(dir.path(), "notes.txt", "n");
    write(dir.path(), "Makefile", "all:");
    write(dir.path(), "bin/tool", "binary");
    write(dir.path(), "trailing.", "dot");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(relative_paths(&result, dir.path()), ["notes.txt"]);
    assert_eq!(result.no_extension, 3, "a name ending in a dot has no extension either");
}

#[test]
fn reports_progress_ending_with_the_final_counts() {
    let dir = scan_root();
    write(dir.path(), "a.txt", "a");
    write(dir.path(), "sub/b.txt", "b");
    write(dir.path(), "sub/deeper/c.txt", "c");
    write(dir.path(), "node_modules/x/index.js", "js");
    let mut reports = Vec::new();

    let result = scan_with_progress(dir.path(), &empty_config(dir.path()), |p| reports.push(p)).unwrap();

    let last = reports.last().expect("at least one progress report");
    assert_eq!(last.found, 3);
    assert_eq!(last.folders, 3, "the Scan Root, sub and sub/deeper");
    assert_eq!(last.skipped, 1);
    assert_eq!(last.current, result.root);
    assert_eq!(result.entries.len(), 3);
}

#[test]
fn never_follows_symlinks() {
    let outside = scan_root();
    write(outside.path(), "elsewhere/secret.txt", "outside the root");
    let dir = scan_root();
    write(dir.path(), "real.txt", "real");
    std::os::unix::fs::symlink(dir.path().join("real.txt"), dir.path().join("link.txt")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("elsewhere"), dir.path().join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).unwrap();

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(relative_paths(&result, dir.path()), ["real.txt"]);
}

#[test]
fn skips_hidden_files_and_folders() {
    let dir = scan_root();
    write(dir.path(), "visible.txt", "v");
    write(dir.path(), ".env", "secret");
    write(dir.path(), ".cache/inside.txt", "c");
    write(dir.path(), "sub/.hidden.txt", "h");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(relative_paths(&result, dir.path()), ["visible.txt"]);
}

#[test]
fn treats_packages_as_one_item_with_their_total_size() {
    let dir = scan_root();
    write(dir.path(), "Tool.app/Contents/MacOS/tool", "0123456789");
    write(dir.path(), "Tool.app/Contents/Info.plist", "plist");
    write(dir.path(), "Tool.app/Contents/.hidden", "abc");
    std::os::unix::fs::symlink("/usr/bin", dir.path().join("Tool.app/Contents/bin")).unwrap();
    write(dir.path(), "school/Essay.pages/Index.zip", "1234");
    write(dir.path(), "school/plain.txt", "p");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(
        relative_paths(&result, dir.path()),
        ["Tool.app", "school/Essay.pages", "school/plain.txt"]
    );
    let app = result.entries.iter().find(|e| e.name == "Tool.app").unwrap();
    assert!(app.is_package);
    assert_eq!(app.extension, "app");
    assert_eq!(app.size, 18);
    let essay = result.entries.iter().find(|e| e.name == "Essay.pages").unwrap();
    assert!(essay.is_package);
    assert_eq!(essay.size, 4);
}

#[test]
fn an_unreadable_folder_is_counted_and_the_scan_carries_on() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scan_root();
    write(dir.path(), "fine.txt", "f");
    write(dir.path(), "locked/inside.txt", "i");
    let locked = dir.path().join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let result = scan(dir.path(), &empty_config(dir.path()));
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let result = result.unwrap();

    assert_eq!(relative_paths(&result, dir.path()), ["fine.txt"]);
    assert_eq!(result.unreadable, [fs::canonicalize(&locked).unwrap()]);
    assert_eq!(result.root, fs::canonicalize(dir.path()).unwrap());
}

#[test]
fn a_package_with_an_unreadable_folder_inside_is_still_listed() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scan_root();
    write(dir.path(), "Tool.app/Contents/Info.plist", "plist");
    write(dir.path(), "Tool.app/Contents/Locked/secret", "secret");
    let locked = dir.path().join("Tool.app/Contents/Locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let result = scan(dir.path(), &empty_config(dir.path()));
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let result = result.unwrap();

    assert_eq!(relative_paths(&result, dir.path()), ["Tool.app"]);
    assert_eq!(result.entries[0].size, 5, "counts what it can read");
}
