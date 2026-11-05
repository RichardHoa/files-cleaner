//! Trash through the OS Trash: where items land, what Undo does with that,
//! and what happens when the OS Trash refuses.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use common::{
    build_plan, empty_config, journal_dir, real_path, run, scan_root, undo, write, FakeTrash,
};
use files_cleaner_core::{self as core, scan, session_details, OperationState, Plan};

/// A Scan Root with `keep.txt` (the Kept Copy) and two older copies.
fn folder_with_copies(root: &Path) {
    for (path, secs) in [
        ("keep.txt", 3_000),
        ("a/copy.txt", 2_000),
        ("b/copy.txt", 1_000),
    ] {
        let time = UNIX_EPOCH + Duration::from_secs(secs);
        fs::File::options()
            .write(true)
            .open(write(root, path, "same contents"))
            .unwrap()
            .set_modified(time)
            .unwrap();
    }
}

fn plan_for(root: &Path) -> Plan {
    let config = empty_config(root);
    build_plan(&scan(root, &config).unwrap(), &config)
}

fn ticked(plan: &Plan) -> Vec<String> {
    let ticked = plan.operations.iter().filter(|op| op.ticked);
    ticked.map(|op| op.id.clone()).collect()
}

/// Where Session `id` says the item trashed from `relative` landed.
fn landed(journal: &Path, id: &str, relative: &str) -> PathBuf {
    let details = session_details(id, journal).unwrap();
    let op = details
        .operations
        .iter()
        .find(|op| op.relative_path.join("/") == relative);
    op.unwrap().destination.clone().unwrap()
}

#[test]
fn undo_puts_a_trashed_file_back_from_where_it_landed() {
    let dir = scan_root();
    let journal = journal_dir();
    folder_with_copies(dir.path());
    let plan = plan_for(dir.path());

    let summary = run(&plan, &ticked(&plan), journal.path()).unwrap();

    assert_eq!(summary.done.len(), 2);
    assert!(!dir.path().join("a/copy.txt").exists());
    let landing = landed(journal.path(), &summary.session_id, "a/copy.txt");
    assert_eq!(fs::read_to_string(&landing).unwrap(), "same contents");

    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(undone.done.len(), 2);
    assert!(!landing.exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("a/copy.txt")).unwrap(),
        "same contents"
    );
}

#[test]
fn a_file_emptied_from_the_trash_is_unrecoverable_and_the_rest_still_undoes() {
    let dir = scan_root();
    let journal = journal_dir();
    folder_with_copies(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &ticked(&plan), journal.path()).unwrap();
    fs::remove_file(landed(journal.path(), &summary.session_id, "a/copy.txt")).unwrap();

    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(undone.unrecoverable.len(), 1);
    let note = undone.unrecoverable[0].note.as_deref().unwrap();
    assert!(note.contains("no longer in the"), "{note}");
    assert_eq!(undone.done.len(), 1);
    assert!(dir.path().join("b/copy.txt").exists());
}

#[test]
fn another_file_landing_under_the_same_name_is_never_restored_in_its_place() {
    let dir = scan_root();
    let journal = journal_dir();
    folder_with_copies(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &ticked(&plan), journal.path()).unwrap();
    // The Trash was emptied, and a later Trash reused the name.
    let landing = landed(journal.path(), &summary.session_id, "a/copy.txt");
    fs::remove_file(&landing).unwrap();
    fs::write(&landing, "another file altogether").unwrap();

    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(undone.unrecoverable.len(), 1, "{undone:?}");
    assert!(!dir.path().join("a/copy.txt").exists());
    assert_eq!(
        fs::read_to_string(&landing).unwrap(),
        "another file altogether"
    );
}

#[test]
fn a_file_the_trash_refuses_stays_in_place_and_the_rest_of_the_plan_runs() {
    let dir = scan_root();
    let journal = journal_dir();
    folder_with_copies(dir.path());
    let plan = plan_for(dir.path());
    let refused = real_path(&dir.path().join("a/copy.txt"));
    let mut trash = FakeTrash::new(journal.path());
    trash.refuses.push(refused.clone());

    let summary = core::run(&plan, &ticked(&plan), journal.path(), &trash).unwrap();

    assert!(refused.exists());
    assert_eq!(summary.skipped.len(), 1);
    assert!(summary.skipped[0]
        .note
        .as_deref()
        .unwrap()
        .contains("refused"));
    assert_eq!(summary.done.len(), 1);
    assert!(!dir.path().join("b/copy.txt").exists());
}

#[test]
fn a_folder_with_no_trash_is_blocked_in_the_plan_and_skipped_with_a_note_at_run_time() {
    let dir = scan_root();
    let journal = journal_dir();
    folder_with_copies(dir.path());
    let config = empty_config(dir.path());
    let mut trash = FakeTrash::new(journal.path());
    trash.blocks.push(real_path(&dir.path().join("a")));

    let plan = core::build_plan(&scan(dir.path(), &config).unwrap(), &config, &trash);

    let blocked = plan
        .operations
        .iter()
        .find(|op| op.relative_path.join("/") == "a/copy.txt")
        .unwrap();
    assert!(blocked.error.as_deref().unwrap().contains("no Trash"));
    assert!(!blocked.ticked);

    let mut approved = ticked(&plan);
    approved.push(blocked.id.clone());
    let summary = core::run(&plan, &approved, journal.path(), &trash).unwrap();

    assert!(dir.path().join("a/copy.txt").exists());
    assert_eq!(summary.skipped.len(), 1);
    assert_eq!(summary.done.len(), 1);
}

#[test]
fn a_crash_before_the_landing_was_recorded_says_to_restore_it_by_hand() {
    let dir = scan_root();
    let journal = journal_dir();
    folder_with_copies(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &ticked(&plan), journal.path()).unwrap();
    // The Journal as the crash left it: intents, and no outcome after them.
    let path = journal.path().join(format!("{}.jsonl", summary.session_id));
    let text = fs::read_to_string(&path).unwrap();
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| !l.contains("\"outcome\"") && !l.contains("finished"))
        .collect();
    fs::write(&path, format!("{}\n", kept.join("\n"))).unwrap();

    let details = session_details(&summary.session_id, journal.path()).unwrap();

    let op = &details.operations[0];
    assert_eq!(op.state, OperationState::Unrecoverable);
    let note = op.note.as_deref().unwrap();
    assert!(
        note.contains("may be in the") && note.contains("by hand"),
        "{note}"
    );

    let undone = undo(&summary.session_id, journal.path()).unwrap();
    assert!(undone.done.is_empty(), "the app never guesses: {undone:?}");
}

#[test]
fn an_old_journal_still_undoes_from_its_recorded_destination() {
    let dir = scan_root();
    let journal = journal_dir();
    let id = "2026-01-01_00-00-00-000Z";
    let root = real_path(dir.path());
    let held = write(&root, &format!("Files Cleaner Trash/{id}/a/x.txt"), "old");
    let source = root.join("a/x.txt");
    let records = [
        serde_json::json!({ "id": id, "scanRoot": root, "trashDestination": "holdingFolder", "startedAt": 0 }),
        serde_json::json!({
            "record": "operation", "operationId": "op", "kind": "trash", "source": source, "destination": held,
            "planned": { "size": 3, "modified": 0, "isPackage": false },
        }),
        serde_json::json!({ "record": "outcome", "operationId": "op", "phase": "run", "outcome": "done", "reason": null }),
        serde_json::json!({ "record": "finished" }),
    ];
    let lines: String = records.iter().map(|r| format!("{r}\n")).collect();
    fs::write(journal.path().join(format!("{id}.jsonl")), lines).unwrap();

    let undone = undo(id, journal.path()).unwrap();

    assert_eq!(undone.done.len(), 1, "{undone:?}");
    assert_eq!(fs::read_to_string(&source).unwrap(), "old");
    assert!(!held.exists());
}
