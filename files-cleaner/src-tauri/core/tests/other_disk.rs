//! A Plan with an item on another disk. Its own test binary: mounting runs
//! `mount` and `umount`, and a child process briefly holds a copy of the
//! parent's open files, Session locks included, so beside other tests it
//! could make their Undo fail as Busy.
#![cfg(target_os = "linux")]

mod common;

use std::collections::BTreeMap;

use common::{folder_plan, journal_dir, run, scan_root, write, OtherDisk};
use files_cleaner_core::{FolderChoice, Plan, Reason};

#[test]
fn moves_across_disks_are_errors_skipped_with_their_note_at_run_time() {
    let dir = scan_root();
    let Some(_disk) = OtherDisk::mount(&dir.path().join("usb")) else {
        eprintln!("skipped: mounting a second disk needs root");
        return;
    };
    write(dir.path(), "usb/talk.pptx", "talk");
    write(dir.path(), "here/intro.pptx", "intro");
    let errors = |plan: &Plan| -> BTreeMap<String, String> {
        plan.operations
            .iter()
            .filter_map(|op| Some((op.relative_path.join("/"), op.error.clone()?)))
            .collect()
    };

    let plan = folder_plan(
        dir.path(),
        "Slides",
        FolderChoice::CreateHere,
        &["usb/talk.pptx", "here/intro.pptx"],
    );
    let moves = errors(&plan);
    assert_eq!(moves.keys().collect::<Vec<_>>(), ["usb/talk.pptx"]);
    assert!(
        moves.values().all(|e| e.contains("different disk")),
        "{moves:?}"
    );
    let blocked: Vec<_> = plan
        .operations
        .iter()
        .filter(|op| op.error.is_some())
        .collect();
    assert!(blocked.iter().all(|op| !op.ticked));
    let emptied: Vec<String> = plan
        .operations
        .iter()
        .filter(|op| op.reason == Reason::EmptiedFolder)
        .map(|op| op.relative_path.join("/"))
        .collect();
    assert_eq!(
        emptied,
        ["here"],
        "a folder whose items can't leave isn't emptied"
    );

    let journal = journal_dir();
    let every_id: Vec<String> = plan.operations.iter().map(|op| op.id.clone()).collect();
    let summary = run(&plan, &every_id, journal.path()).unwrap();

    for op in &blocked {
        let skipped = summary.skipped.iter().find(|o| o.operation_id == op.id);
        assert_eq!(
            skipped.and_then(|o| o.note.as_deref()),
            op.error.as_deref(),
            "{summary:?}"
        );
    }
    assert!(dir.path().join("usb/talk.pptx").exists());
    assert!(
        !dir.path().join("here").exists(),
        "the rest of the Plan runs"
    );
}
