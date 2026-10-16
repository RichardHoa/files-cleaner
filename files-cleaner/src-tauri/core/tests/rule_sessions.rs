mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use common::{config, scan_root, write};
use files_cleaner_core::{
    build_plan, list_sessions, redo_operations, run, scan, undo, undo_operations, OperationKind, Plan, SessionStatus,
};
use tempfile::TempDir;

/// Every file and folder under `root` (relative, `/`-separated), with each
/// file's contents.
fn snapshot(root: &Path) -> BTreeMap<String, Option<String>> {
    let mut found = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for child in fs::read_dir(&folder).unwrap() {
            let path = child.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if path.is_dir() {
                found.insert(relative, None);
                folders.push(path);
            } else {
                found.insert(relative, Some(fs::read_to_string(&path).unwrap()));
            }
        }
    }
    found
}

/// Slides go to `Study/Slides`, renamed `old {name}`; disk images are trashed.
const RULES: &str = r#"{ "rules": [
  { "name": "Slides", "match": { "extensions": ["pptx"] }, "then": { "moveTo": "Study/Slides", "rename": "old {name}" } },
  { "name": "Disk images", "match": { "extensions": ["dmg"] }, "then": { "trash": true } }
] }"#;

/// A Scan Root with two slide decks, a disk image and an unrelated file.
fn messy_folder() -> TempDir {
    let dir = scan_root();
    write(dir.path(), "talk.pptx", "talk");
    write(dir.path(), "sub/lecture.pptx", "lecture");
    write(dir.path(), "setup.dmg", "installer");
    write(dir.path(), "notes.txt", "notes");
    dir
}

fn plan_for(root: &Path) -> Plan {
    let config = config(RULES, root);
    build_plan(&scan(root, &config).unwrap(), &config)
}

fn all_ids(plan: &Plan) -> Vec<String> {
    plan.operations.iter().map(|op| op.id.clone()).collect()
}

fn ids_except(plan: &Plan, kind: OperationKind) -> Vec<String> {
    plan.operations.iter().filter(|op| op.kind != kind).map(|op| op.id.clone()).collect()
}

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).unwrap()
}

#[test]
fn run_creates_folders_moves_and_renames_and_undo_restores_the_original_tree() {
    let dir = messy_folder();
    let journal = TempDir::new().unwrap();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());

    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    assert_eq!(summary.done.len(), 5, "two folders, two moves and a trash: {summary:?}");
    assert_eq!(read(dir.path(), "Study/Slides/old talk.pptx"), "talk");
    assert_eq!(read(dir.path(), "Study/Slides/old lecture.pptx"), "lecture");
    assert!(!dir.path().join("talk.pptx").exists() && !dir.path().join("setup.dmg").exists());
    let held = format!("Files Cleaner Trash/{}/setup.dmg", summary.session_id);
    assert_eq!(read(dir.path(), &held), "installer");

    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(undone.done.len(), 5, "{undone:?}");
    assert_eq!(snapshot(dir.path()), before, "the created folders are gone too");
    assert_eq!(list_sessions(journal.path(), 0, 1).unwrap().sessions[0].status, SessionStatus::Undone);
}

#[test]
fn undo_leaves_a_created_folder_that_is_no_longer_empty() {
    let dir = messy_folder();
    let journal = TempDir::new().unwrap();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    write(dir.path(), "Study/Slides/mine.txt", "added since");

    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(read(dir.path(), "Study/Slides/mine.txt"), "added since");
    assert_eq!(read(dir.path(), "talk.pptx"), "talk", "the moves are still undone");
    let skipped: Vec<_> = undone.skipped.iter().map(|o| o.destination.clone()).collect();
    let root = &plan.root;
    assert_eq!(skipped, [root.join("Study/Slides"), root.join("Study")]);
    assert!(undone.skipped.iter().all(|o| o.note.as_deref().unwrap().contains("isn't empty")), "{undone:?}");
}

#[test]
fn a_destination_taken_after_planning_gets_a_new_clash_suffix_at_run_time() {
    let dir = messy_folder();
    let journal = TempDir::new().unwrap();
    let plan = plan_for(dir.path());
    write(dir.path(), "Study/Slides/OLD TALK.pptx", "got there first");

    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    assert_eq!(read(dir.path(), "Study/Slides/OLD TALK.pptx"), "got there first");
    assert_eq!(read(dir.path(), "Study/Slides/old talk (1).pptx"), "talk");
    let created: Vec<_> = summary.skipped.iter().map(|o| o.kind).collect();
    assert_eq!(created, [OperationKind::CreateFolder, OperationKind::CreateFolder], "they already exist");

    undo(&summary.session_id, journal.path()).unwrap();
    assert_eq!(read(dir.path(), "talk.pptx"), "talk");
    assert_eq!(read(dir.path(), "Study/Slides/OLD TALK.pptx"), "got there first", "folders it didn't create stay");
}

#[test]
fn a_move_into_a_folder_whose_creation_was_unticked_is_skipped() {
    let dir = messy_folder();
    let journal = TempDir::new().unwrap();
    let plan = plan_for(dir.path());

    let summary = run(&plan, &ids_except(&plan, OperationKind::CreateFolder), journal.path()).unwrap();

    assert!(!dir.path().join("Study").exists(), "no folder is created without its Operation");
    assert_eq!(read(dir.path(), "talk.pptx"), "talk");
    let moves: Vec<_> = summary.failed.iter().chain(&summary.skipped).filter(|o| o.kind == OperationKind::Move).collect();
    assert_eq!(moves.len(), 2, "{summary:?}");
    assert!(moves.iter().all(|o| o.note.as_deref().unwrap().contains("doesn't exist")), "{summary:?}");
}

#[test]
fn undo_never_overwrites_a_file_now_at_a_moved_files_original_location() {
    let dir = messy_folder();
    let journal = TempDir::new().unwrap();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    write(dir.path(), "talk.pptx", "a new talk");

    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(read(dir.path(), "talk.pptx"), "a new talk");
    assert_eq!(read(dir.path(), "Study/Slides/old talk.pptx"), "talk", "the moved file stays where it was moved");
    let skipped: Vec<_> = undone.skipped.iter().map(|o| o.kind).collect();
    assert!(skipped.contains(&OperationKind::Move), "{undone:?}");
    assert_eq!(read(dir.path(), "sub/lecture.pptx"), "lecture", "the rest is undone");
}

#[test]
fn undo_reports_a_moved_file_that_is_gone_as_unrecoverable() {
    let dir = messy_folder();
    let journal = TempDir::new().unwrap();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    fs::remove_file(dir.path().join("Study/Slides/old talk.pptx")).unwrap();

    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(undone.unrecoverable.len(), 1);
    assert!(undone.unrecoverable[0].note.as_deref().unwrap().contains("gone"));
    assert_eq!(read(dir.path(), "sub/lecture.pptx"), "lecture");
}

#[test]
fn redo_moves_and_renames_again_after_undo() {
    let dir = messy_folder();
    let journal = TempDir::new().unwrap();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    undo(&summary.session_id, journal.path()).unwrap();
    let after_run_ids: Vec<String> = summary.done.iter().map(|o| o.operation_id.clone()).collect();

    let redone = redo_operations(&summary.session_id, &after_run_ids, journal.path()).unwrap();

    assert_eq!(redone.done.len(), 5, "{redone:?}");
    assert_eq!(read(dir.path(), "Study/Slides/old talk.pptx"), "talk");

    let only_folder = vec![plan.operations[1].id.clone()];
    let partly = undo_operations(&summary.session_id, &only_folder, journal.path()).unwrap();
    assert_eq!(partly.skipped.len(), 1, "a folder with files in it stays: {partly:?}");
}
