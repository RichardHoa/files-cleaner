//! Ignored by default, so a `cargo test` never fills the developer's Trash; CI
//! runs them with `-- --ignored`.

mod common;

use std::fs;

use common::{empty_config, journal_dir, scan_root, write};
use files_cleaner_core::{self as core, scan, SystemTrash};

#[test]
#[ignore = "uses the real System Trash; run on CI"]
fn a_run_trashes_a_copy_to_the_system_trash_and_undo_puts_it_back() {
    let dir = scan_root();
    let journal = journal_dir();
    write(dir.path(), "keep.txt", "same contents");
    let copy = write(dir.path(), "a/copy.txt", "same contents");
    // Older, so it's the copy that's trashed.
    fs::File::options()
        .write(true)
        .open(&copy)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH)
        .unwrap();
    let config = empty_config(dir.path());
    let plan = core::build_plan(&scan(dir.path(), &config).unwrap(), &config, &SystemTrash);
    let ticked: Vec<String> = plan
        .operations
        .iter()
        .filter(|op| op.ticked)
        .map(|op| op.id.clone())
        .collect();

    let summary = core::run(&plan, &ticked, journal.path(), &SystemTrash).unwrap();

    assert_eq!(summary.done.len(), 1, "{summary:?}");
    assert!(!copy.exists());
    let landed = summary.done[0].destination.clone().unwrap();
    assert_eq!(fs::read_to_string(&landed).unwrap(), "same contents");

    let id = summary.session_id.unwrap();
    let undone = core::undo(&id, journal.path(), &SystemTrash).unwrap();

    assert_eq!(undone.done.len(), 1, "{undone:?}");
    assert!(!landed.exists());
    assert_eq!(fs::read_to_string(&copy).unwrap(), "same contents");
}

/// An item on another disk goes to that disk's own Trash, recorded relative to
/// its top.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "uses the real System Trash; run on CI"]
fn on_another_disk_an_item_goes_to_that_disks_trash() {
    use files_cleaner_core::OsTrash;
    use std::path::Path;

    let dir = scan_root();
    let Some(disk) = common::OtherDisk::mount(&dir.path().join("usb")) else {
        eprintln!("skipped: mounting a second disk needs root");
        return;
    };
    let file = common::real_path(&write(disk.folder(), "photos/copy.txt", "contents"));

    let landed = SystemTrash.trash(&file).unwrap();

    let trash = landed.parent().and_then(Path::parent).unwrap();
    assert!(trash.starts_with(disk.folder()), "{}", landed.display());
    let info = trash.join("info").join("copy.txt.trashinfo");
    assert!(fs::read_to_string(&info)
        .unwrap()
        .contains("\nPath=photos/copy.txt\n"));

    SystemTrash.put_back(&landed, &file).unwrap();

    assert_eq!(fs::read_to_string(&file).unwrap(), "contents");
    assert!(!info.exists());
}
