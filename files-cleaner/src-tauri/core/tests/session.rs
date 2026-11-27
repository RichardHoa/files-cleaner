mod common;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use common::{
    build_plan, build_suggested_folders_plan, drop_outcomes, empty_config, journal_database,
    journal_dir, real_path, redo_operations, retry_operations, run, run_with_progress, scan_root,
    undo, undo_operations, undo_with_progress, write, FakeTrash,
};
use files_cleaner_core::{
    self as core, delete_all_sessions, list_sessions, lock_sessions, scan, session_details,
    FileChoice, FolderChoice, OperationKind, OperationState, OsTrash, Plan, RunPhase,
    SessionDetails, SessionError, SessionInfo, SessionStatus, SuggestedFolderChoice,
};
use tempfile::TempDir;

fn set_modified(path: &Path, secs: u64) {
    let time = UNIX_EPOCH + Duration::from_secs(secs);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

fn snapshot(root: &Path) -> BTreeMap<String, Option<(String, i64)>> {
    let mut found = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for child in fs::read_dir(&folder).unwrap() {
            let path = child.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                found.insert(relative, None);
                folders.push(path);
            } else {
                let modified = fs::metadata(&path).unwrap().modified().unwrap();
                let millis = modified.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
                found.insert(relative, Some((fs::read_to_string(&path).unwrap(), millis)));
            }
        }
    }
    found
}

fn messy_folder() -> TempDir {
    let dir = scan_root();
    for (path, secs) in [
        ("keep.txt", 3_000),
        ("a/copy.txt", 2_000),
        ("b/c/copy.txt", 1_000),
    ] {
        set_modified(&write(dir.path(), path, "same contents"), secs);
    }
    write(dir.path(), "other.pdf", "different");
    dir
}

fn plan_for(root: &Path) -> Plan {
    let config = empty_config(root);
    build_plan(&scan(root, &config).unwrap(), &config)
}

fn id_of(plan: &Plan, relative: &str) -> String {
    plan.operations
        .iter()
        .find(|op| op.relative_path.join("/") == relative)
        .unwrap()
        .id
        .clone()
}

fn landed(journal: &Path, id: &str, relative: &str) -> PathBuf {
    let details = session_details(id, journal).unwrap();
    let op = details
        .operations
        .iter()
        .find(|op| op.relative_path.join("/") == relative);
    op.unwrap().destination.clone().unwrap()
}

fn all_sessions(journal: &Path) -> Vec<SessionInfo> {
    list_sessions(journal, 0, usize::MAX).unwrap().sessions
}

fn all_ids(plan: &Plan) -> Vec<String> {
    plan.operations
        .iter()
        .filter(|op| op.ticked)
        .map(|op| op.id.clone())
        .collect()
}

#[test]
fn run_trashes_only_approved_operations() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());

    let summary = run(&plan, &[id_of(&plan, "b/c/copy.txt")], journal.path()).unwrap();

    assert_eq!(
        fs::read_to_string(landed(
            journal.path(),
            summary.session_id.as_deref().unwrap(),
            "b/c/copy.txt"
        ))
        .unwrap(),
        "same contents"
    );
    assert!(!dir.path().join("b/c/copy.txt").exists());
    assert!(
        dir.path().join("a/copy.txt").exists(),
        "unapproved Operations don't run"
    );
    assert_eq!(summary.done.len(), 1);
    assert!(summary.skipped.is_empty() && summary.failed.is_empty());
}

#[test]
fn a_run_reports_its_progress_ending_with_every_operation_finished() {
    let dir = scan_root();
    set_modified(&write(dir.path(), "keep.txt", "same"), 9_000);
    for i in 0..300 {
        set_modified(
            &write(
                dir.path(),
                &format!("copies/{}/copy {i}.txt", i % 7),
                "same",
            ),
            1_000,
        );
    }
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let approved = all_ids(&plan);
    assert_eq!(approved.len(), 300);

    let mut reports = Vec::new();
    let summary = run_with_progress(&plan, &approved, journal.path(), |p| reports.push(p)).unwrap();

    assert_eq!(summary.done.len(), 300);
    assert!(reports
        .iter()
        .all(|p| p.total == 300 && p.finished <= p.total));
    assert!(
        reports.windows(2).all(|w| w[0].finished <= w[1].finished),
        "never goes backwards"
    );
    assert_eq!(
        reports[0].finished, 0,
        "the first report comes before anything moves"
    );
    let last = reports.last().unwrap();
    assert_eq!(
        (last.finished, last.current.as_path()),
        (300, plan.root.as_path())
    );
}

#[test]
fn an_undo_reports_its_progress_ending_with_every_operation_finished() {
    let dir = scan_root();
    set_modified(&write(dir.path(), "keep.txt", "same"), 9_000);
    for i in 0..50 {
        set_modified(&write(dir.path(), &format!("copy {i}.txt"), "same"), 1_000);
    }
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let mut reports = Vec::new();
    let undone = undo_with_progress(
        summary.session_id.as_deref().unwrap(),
        journal.path(),
        |p| reports.push(p),
    )
    .unwrap();

    assert_eq!(undone.done.len(), 50);
    assert_eq!(reports[0].finished, 0);
    assert!(reports.windows(2).all(|w| w[0].finished <= w[1].finished));
    let last = reports.last().unwrap();
    assert_eq!((last.finished, last.total), (50, 50));
}

/// The user clicks Cancel while the `at`-th copy is going to the Trash.
struct CancelsAt<'a> {
    trash: FakeTrash,
    at: usize,
    trashed: Cell<usize>,
    cancel: &'a AtomicBool,
}

impl OsTrash for CancelsAt<'_> {
    fn blocked(&self, folder: &Path) -> Option<String> {
        self.trash.blocked(folder)
    }

    fn trash(&self, path: &Path) -> Result<PathBuf, String> {
        self.trashed.set(self.trashed.get() + 1);
        if self.trashed.get() == self.at {
            self.cancel.store(true, Ordering::Relaxed);
        }
        self.trash.trash(path)
    }

    fn put_back(&self, landed: &Path, to: &Path) -> Result<(), String> {
        self.trash.put_back(landed, to)
    }
}

#[test]
fn a_run_cancelled_part_way_puts_everything_back_and_keeps_no_session() {
    let dir = scan_root();
    set_modified(&write(dir.path(), "keep.txt", "same"), 9_000);
    for i in 0..10 {
        set_modified(&write(dir.path(), &format!("copy {i}.txt"), "same"), 1_000);
    }
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let cancel = AtomicBool::new(false);
    let trash = CancelsAt {
        trash: FakeTrash::new(journal.path()),
        at: 4,
        trashed: Cell::new(0),
        cancel: &cancel,
    };

    let mut reports = Vec::new();
    let summary = core::run_with_progress(
        &plan,
        &all_ids(&plan),
        journal.path(),
        &trash,
        &cancel,
        |p| reports.push(p),
    )
    .unwrap();

    assert!(summary.cancelled);
    assert_eq!(
        trash.trashed.get(),
        4,
        "the copy under way finishes, no more start"
    );
    assert_eq!(summary.done.len(), 4, "every copy trashed goes back");
    assert_eq!(summary.session_id, None);
    assert!(all_sessions(journal.path()).is_empty());
    assert_eq!(snapshot(dir.path()), before);
    let last = reports.last().unwrap();
    assert_eq!(
        (last.phase, last.finished, last.total),
        (RunPhase::PuttingBack, 4, 4)
    );
}

#[test]
fn a_cancelled_plan_moves_files_back_and_removes_the_folder_it_made() {
    let dir = scan_root();
    // The Plan runs in path order, so the trashes come after the moves.
    for name in ["lecture.pdf", "slides.pptx", "x-junk.txt", "y-junk.txt"] {
        write(dir.path(), name, name);
    }
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let scan = scan(dir.path(), &empty_config(dir.path())).unwrap();
    let folder = |name: &str, choice, files: [&str; 2]| SuggestedFolderChoice {
        name: name.to_string(),
        choice,
        files: files
            .map(|file| (scan.root.join(file), FileChoice::InFolder))
            .to_vec(),
    };
    let plan = build_suggested_folders_plan(
        &scan,
        &[
            folder(
                "CS101",
                FolderChoice::CreateHere,
                ["lecture.pdf", "slides.pptx"],
            ),
            folder("Old", FolderChoice::TrashAll, ["x-junk.txt", "y-junk.txt"]),
        ],
    )
    .unwrap();
    let cancel = AtomicBool::new(false);
    let trash = CancelsAt {
        trash: FakeTrash::new(journal.path()),
        at: 1,
        trashed: Cell::new(0),
        cancel: &cancel,
    };

    let summary = core::run_with_progress(
        &plan,
        &all_ids(&plan),
        journal.path(),
        &trash,
        &cancel,
        |_| {},
    )
    .unwrap();

    assert!(summary.cancelled);
    assert_eq!(
        summary.done.len(),
        4,
        "the folder, both moves and one trash"
    );
    assert_eq!(snapshot(dir.path()), before);
    assert!(all_sessions(journal.path()).is_empty());
}

#[test]
fn a_cancelled_folder_trash_now_puts_back_every_file_already_trashed() {
    let dir = scan_root();
    for name in ["a.txt", "b.txt", "c.txt"] {
        write(dir.path(), name, name);
    }
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let scan = scan(dir.path(), &empty_config(dir.path())).unwrap();
    let folder = SuggestedFolderChoice {
        name: "Old".to_string(),
        choice: FolderChoice::TrashAll,
        files: ["a.txt", "b.txt", "c.txt"]
            .map(|file| (scan.root.join(file), FileChoice::Trash))
            .to_vec(),
    };
    let plan = build_suggested_folders_plan(&scan, &[folder]).unwrap();
    let cancel = AtomicBool::new(false);
    let trash = CancelsAt {
        trash: FakeTrash::new(journal.path()),
        at: 2,
        trashed: Cell::new(0),
        cancel: &cancel,
    };

    let summary = core::run_with_progress(
        &plan,
        &all_ids(&plan),
        journal.path(),
        &trash,
        &cancel,
        |_| {},
    )
    .unwrap();

    assert!(summary.cancelled);
    assert_eq!(summary.done.len(), 2);
    assert_eq!(snapshot(dir.path()), before);
    assert!(all_sessions(journal.path()).is_empty());
}

#[test]
fn a_cancel_after_the_run_finished_finds_the_flag_set_and_puts_nothing_back() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let cancel = AtomicBool::new(false);

    let summary = core::run_with_progress(
        &plan,
        &all_ids(&plan),
        journal.path(),
        &FakeTrash::new(journal.path()),
        &cancel,
        |_| {},
    )
    .unwrap();

    assert!(!summary.cancelled);
    assert_eq!(summary.done.len(), 2);
    assert!(
        cancel.swap(true, Ordering::Relaxed),
        "a later Cancel is told it came too late"
    );
    assert_eq!(all_sessions(journal.path()).len(), 1);
}

#[test]
fn a_source_that_changed_after_planning_is_skipped_with_a_note() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    set_modified(&dir.path().join("a/copy.txt"), 9_000);
    fs::remove_file(dir.path().join("b/c/copy.txt")).unwrap();

    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    assert!(summary.done.is_empty());
    assert_eq!(summary.skipped.len(), 2);
    let notes: Vec<&str> = summary
        .skipped
        .iter()
        .map(|o| o.note.as_deref().unwrap())
        .collect();
    assert!(notes.iter().any(|n| n.contains("modified")), "{notes:?}");
    assert!(
        notes.iter().any(|n| n.contains("no longer exists")),
        "{notes:?}"
    );
    assert!(dir.path().join("a/copy.txt").exists());
}

#[test]
fn a_source_whose_size_changed_is_skipped() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let path = dir.path().join("a/copy.txt");
    fs::write(&path, "longer contents now").unwrap();
    set_modified(&path, 2_000);

    let summary = run(&plan, &[id_of(&plan, "a/copy.txt")], journal.path()).unwrap();

    assert!(summary.skipped[0].note.as_deref().unwrap().contains("size"));
    assert!(path.exists());
}

#[test]
fn scan_plan_run_undo_restores_the_original_folder_exactly() {
    let dir = messy_folder();
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());

    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    assert_eq!(summary.done.len(), 2);
    assert_ne!(snapshot(dir.path()), before);

    // Only knows the Journal folder, as after a restart.
    let journal_dir: PathBuf = journal.path().to_path_buf();
    let undone = undo(summary.session_id.as_deref().unwrap(), &journal_dir).unwrap();

    assert_eq!(undone.done.len(), 2);
    assert!(undone.skipped.is_empty() && undone.unrecoverable.is_empty());
    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn undo_never_overwrites_a_file_now_at_the_original_location() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    write(dir.path(), "a/copy.txt", "something new");

    let undone = undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(
        fs::read_to_string(dir.path().join("a/copy.txt")).unwrap(),
        "something new"
    );
    assert_eq!(undone.skipped.len(), 1);
    assert!(undone.skipped[0]
        .note
        .as_deref()
        .unwrap()
        .contains("already"));
    assert_eq!(
        undone.done.len(),
        1,
        "the rest of the Session is still undone"
    );
    assert_eq!(
        fs::read_to_string(landed(
            journal.path(),
            summary.session_id.as_deref().unwrap(),
            "a/copy.txt"
        ))
        .unwrap(),
        "same contents",
        "the trashed copy stays put"
    );
}

#[test]
fn undo_reports_a_trashed_copy_that_is_gone_as_unrecoverable() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    fs::remove_file(landed(
        journal.path(),
        summary.session_id.as_deref().unwrap(),
        "a/copy.txt",
    ))
    .unwrap();

    let undone = undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(undone.unrecoverable.len(), 1);
    assert_eq!(undone.done.len(), 1);
    assert!(dir.path().join("b/c/copy.txt").exists());
    let session = &all_sessions(journal.path())[0];
    assert_eq!(session.status, SessionStatus::PartlyUndone);
    assert_eq!(
        session.left_to_undo, 0,
        "nothing is left that Undo could reverse"
    );
}

#[test]
fn sessions_are_listed_newest_first_with_their_status() {
    let journal = journal_dir();

    let first = messy_folder();
    let plan = plan_for(first.path());
    let completed = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let second = messy_folder();
    let plan = plan_for(second.path());
    let undone = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    undo(undone.session_id.as_deref().unwrap(), journal.path()).unwrap();

    let third = messy_folder();
    let plan = plan_for(third.path());
    let partly = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    write(third.path(), "a/copy.txt", "in the way");
    undo(partly.session_id.as_deref().unwrap(), journal.path()).unwrap();

    let sessions = all_sessions(journal.path());
    let ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            partly.session_id.as_deref().unwrap(),
            undone.session_id.as_deref().unwrap(),
            completed.session_id.as_deref().unwrap()
        ]
    );
    assert_eq!(sessions[0].status, SessionStatus::PartlyUndone);
    assert_eq!(sessions[1].status, SessionStatus::Undone);
    assert_eq!(sessions[2].status, SessionStatus::Completed);
    assert_eq!(sessions[2].scan_root, real_path(first.path()));
    assert_eq!(sessions[2].done, 2);
}

#[test]
fn undoing_a_partly_undone_session_again_retries_what_was_skipped() {
    let dir = messy_folder();
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    let blocker = write(dir.path(), "a/copy.txt", "in the way");
    undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();
    fs::remove_file(blocker).unwrap();

    let again = undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(again.done.len(), 1);
    assert_eq!(snapshot(dir.path()), before);
    assert_eq!(
        all_sessions(journal.path())[0].status,
        SessionStatus::Undone
    );
}

#[test]
fn only_one_session_runs_at_a_time() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let _held = lock_sessions(journal.path()).unwrap();

    assert!(matches!(
        run(&plan, &all_ids(&plan), journal.path()),
        Err(SessionError::Busy)
    ));
    assert!(matches!(
        undo(summary.session_id.as_deref().unwrap(), journal.path()),
        Err(SessionError::Busy)
    ));
}

#[test]
fn deleting_all_sessions_empties_the_list_and_leaves_the_files_where_they_are() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    let trashed = landed(
        journal.path(),
        summary.session_id.as_deref().unwrap(),
        "b/c/copy.txt",
    );
    let before = snapshot(dir.path());

    assert_eq!(delete_all_sessions(journal.path()).unwrap(), 1);

    assert!(all_sessions(journal.path()).is_empty());
    assert_eq!(snapshot(dir.path()), before);
    assert_eq!(fs::read_to_string(trashed).unwrap(), "same contents");
}

#[test]
fn a_damaged_journal_doesnt_hide_the_other_sessions() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    // An old Journal file left empty by a crash isn't imported.
    fs::write(journal.path().join("1999-01-01_00-00-00-000Z.jsonl"), "").unwrap();

    let sessions = all_sessions(journal.path());

    assert_eq!(sessions.len(), 1);
    assert_eq!(Some(sessions[0].id.clone()), summary.session_id);
}

#[test]
fn deleting_every_session_deletes_an_old_journal_that_couldnt_be_imported() {
    let journal = journal_dir();
    let damaged = journal.path().join("1999-01-01_00-00-00-000Z.jsonl");
    fs::write(&damaged, "").unwrap();

    assert_eq!(delete_all_sessions(journal.path()).unwrap(), 0);

    assert!(!damaged.exists());
}

#[test]
fn deleting_every_session_starts_a_damaged_journal_again() {
    let dir = messy_folder();
    let journal = journal_dir();
    fs::write(journal.path().join("journal.sqlite"), "not a database").unwrap();

    delete_all_sessions(journal.path()).unwrap();

    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    assert_eq!(all_sessions(journal.path()).len(), 1);
    assert!(summary.session_id.is_some());
}

fn states(journal: &Path, root: &Path, id: &str) -> Vec<(String, OperationState)> {
    let root = real_path(root);
    session_details(id, journal)
        .unwrap()
        .operations
        .into_iter()
        .map(|op| {
            (
                op.source
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                op.state,
            )
        })
        .collect()
}

#[test]
fn session_details_list_every_operation_with_its_state_size_and_note() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    fs::remove_file(dir.path().join("b/c/copy.txt")).unwrap();
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let details = session_details(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(Some(details.info.id), summary.session_id);
    assert_eq!(
        states(
            journal.path(),
            dir.path(),
            summary.session_id.as_deref().unwrap()
        ),
        [
            ("a/copy.txt".to_string(), OperationState::Done),
            ("b/c/copy.txt".to_string(), OperationState::Skipped)
        ]
    );
    assert_eq!(details.operations[0].size, 13);
    assert_eq!(
        details.operations[0].destination,
        summary.done[0].destination
    );
    assert!(details.operations[1]
        .note
        .as_deref()
        .unwrap()
        .contains("no longer exists"));
}

fn names(path: &str) -> Vec<String> {
    path.split('/').map(String::from).collect()
}

#[test]
fn session_details_give_each_operation_its_paths_from_the_scan_root() {
    let dir = scan_root();
    set_modified(&write(dir.path(), "keep/top.txt", "same contents"), 3_000);
    set_modified(&write(dir.path(), "top.txt", "same contents"), 2_000);
    set_modified(&write(dir.path(), "a/b/copy.txt", "same contents"), 1_000);
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let details = session_details(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    let paths: Vec<Vec<String>> = details
        .operations
        .iter()
        .map(|op| op.relative_path.clone())
        .collect();
    assert_eq!(paths, [names("a/b/copy.txt"), names("top.txt")]);
    let root = real_path(dir.path());
    assert_eq!(
        details.operations[1].source,
        root.join("top.txt"),
        "the absolute paths stay"
    );
    assert!(details.operations[1].in_system_trash);
    assert_eq!(
        details.operations[1].destination,
        summary.done[1].destination
    );
}

#[test]
fn a_journal_written_before_notes_still_reads_with_its_reason_as_the_note() {
    let journal = journal_dir();
    let id = "2026-01-01_00-00-00-000Z";
    let held = format!("/root/Files Cleaner Trash/{id}/a/x.txt");
    let records = [
        serde_json::json!({ "id": id, "scanRoot": "/root", "trashDestination": "holdingFolder", "startedAt": 0 }),
        serde_json::json!({
            "record": "operation", "operationId": "op", "kind": "trash", "source": "/root/a/x.txt", "destination": held,
            "planned": { "size": 4, "modified": 0, "isPackage": false },
        }),
        serde_json::json!({ "record": "outcome", "operationId": "op", "phase": "run", "outcome": "skipped", "reason": "It no longer exists" }),
        serde_json::json!({ "record": "finished" }),
    ];
    let lines: String = records.iter().map(|r| format!("{r}\n")).collect();
    fs::write(journal.path().join(format!("{id}.jsonl")), lines).unwrap();

    let details = session_details(id, journal.path()).unwrap();

    let op = &details.operations[0];
    assert_eq!(op.state, OperationState::Skipped);
    assert_eq!(op.note.as_deref(), Some("It no longer exists"));
    assert_eq!(op.relative_path, names("a/x.txt"));
    assert_eq!(op.destination, Some(PathBuf::from(held)));
    assert!(!op.in_system_trash);
}

#[test]
fn undoing_one_operation_restores_only_that_file() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let undone = undo_operations(
        summary.session_id.as_deref().unwrap(),
        &[id_of(&plan, "a/copy.txt")],
        journal.path(),
    )
    .unwrap();

    assert_eq!(undone.done.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("a/copy.txt")).unwrap(),
        "same contents"
    );
    assert!(!dir.path().join("b/c/copy.txt").exists());
    assert_eq!(
        states(
            journal.path(),
            dir.path(),
            summary.session_id.as_deref().unwrap()
        ),
        [
            ("a/copy.txt".to_string(), OperationState::Undone),
            ("b/c/copy.txt".to_string(), OperationState::Done)
        ]
    );
    assert_eq!(
        all_sessions(journal.path())[0].status,
        SessionStatus::PartlyUndone
    );
}

#[test]
fn redo_trashes_an_undone_file_again() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    let after_run = snapshot(dir.path());
    undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    let redone = redo_operations(
        summary.session_id.as_deref().unwrap(),
        &all_ids(&plan),
        journal.path(),
    )
    .unwrap();

    assert_eq!(redone.done.len(), 2);
    assert!(redone.skipped.is_empty());
    assert_eq!(snapshot(dir.path()), after_run, "each file has left again");
    assert_eq!(
        all_sessions(journal.path())[0].status,
        SessionStatus::Completed
    );
}

#[test]
fn undo_and_redo_can_go_back_and_forth() {
    let dir = messy_folder();
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    let id = [id_of(&plan, "a/copy.txt")];

    for _ in 0..2 {
        undo_operations(summary.session_id.as_deref().unwrap(), &id, journal.path()).unwrap();
        redo_operations(summary.session_id.as_deref().unwrap(), &id, journal.path()).unwrap();
    }
    undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(snapshot(dir.path()), before);
    assert_eq!(
        all_sessions(journal.path())[0].status,
        SessionStatus::Undone
    );
}

#[test]
fn redo_skips_a_file_that_changed_after_it_was_restored() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();
    fs::write(dir.path().join("a/copy.txt"), "edited since").unwrap();

    let redone = redo_operations(
        summary.session_id.as_deref().unwrap(),
        &all_ids(&plan),
        journal.path(),
    )
    .unwrap();

    assert_eq!(redone.done.len(), 1);
    assert_eq!(redone.skipped.len(), 1);
    assert!(redone.skipped[0]
        .note
        .as_deref()
        .unwrap()
        .contains("changed"));
    assert_eq!(
        fs::read_to_string(dir.path().join("a/copy.txt")).unwrap(),
        "edited since"
    );
}

#[test]
fn undo_and_redo_leave_operations_in_the_wrong_state_alone() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let redone = redo_operations(
        summary.session_id.as_deref().unwrap(),
        &all_ids(&plan),
        journal.path(),
    )
    .unwrap();
    assert!(
        redone.done.is_empty() && redone.skipped.is_empty(),
        "nothing was undone yet"
    );

    undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();
    let undone = undo_operations(
        summary.session_id.as_deref().unwrap(),
        &all_ids(&plan),
        journal.path(),
    )
    .unwrap();
    assert!(
        undone.done.is_empty() && undone.skipped.is_empty(),
        "everything is already undone"
    );
}

fn fake_journal(journal: &Path, id: &str, size: u64) {
    let records = [
        serde_json::json!({ "id": id, "scanRoot": "/root", "trashDestination": "holdingFolder", "startedAt": 0 }),
        serde_json::json!({
            "record": "operation", "operationId": "op", "kind": "trash", "source": "/root/a", "destination": "/root/b",
            "planned": { "size": size, "modified": 0, "isPackage": false },
        }),
        serde_json::json!({ "record": "outcome", "operationId": "op", "phase": "run", "outcome": "done", "reason": null }),
        serde_json::json!({ "record": "finished" }),
    ];
    let lines: String = records.iter().map(|r| format!("{r}\n")).collect();
    fs::create_dir_all(journal).unwrap();
    fs::write(journal.join(format!("{id}.jsonl")), lines).unwrap();
}

fn fake_journals(journal: &Path, count: usize) -> Vec<String> {
    (0..count)
        .map(|i| {
            let id = format!("2026-01-01_00-{:02}-{:02}-000Z", i / 60, i % 60);
            fake_journal(journal, &id, i as u64);
            id
        })
        .collect()
}

fn page_ids(journal: &Path, offset: usize, limit: usize) -> Vec<String> {
    list_sessions(journal, offset, limit)
        .unwrap()
        .sessions
        .into_iter()
        .map(|s| s.id)
        .collect()
}

#[test]
fn sessions_are_listed_a_page_at_a_time_newest_first() {
    let journal = journal_dir();
    let mut ids = fake_journals(journal.path(), 23);
    ids.reverse();

    let first = list_sessions(journal.path(), 0, 10).unwrap();

    assert_eq!(first.total, 23);
    assert_eq!(
        first
            .sessions
            .iter()
            .map(|s| s.id.clone())
            .collect::<Vec<_>>(),
        ids[0..10]
    );
    assert_eq!(page_ids(journal.path(), 10, 10), ids[10..20]);
    assert_eq!(
        page_ids(journal.path(), 20, 10),
        ids[20..23],
        "the last page is partial"
    );
    assert!(
        page_ids(journal.path(), 30, 10).is_empty(),
        "an offset past the end is empty"
    );
    assert_eq!(list_sessions(journal.path(), 30, 10).unwrap().total, 23);
}

#[test]
fn no_journals_is_an_empty_first_page() {
    let journal = journal_dir();

    let page = list_sessions(&journal.path().join("not yet"), 0, 10).unwrap();

    assert_eq!(page.total, 0);
    assert!(page.sessions.is_empty());
}

#[test]
fn an_old_journal_that_cant_be_read_is_left_out_of_the_pages() {
    let journal = journal_dir();
    let mut ids = fake_journals(journal.path(), 12);
    ids.reverse();
    fs::write(journal.path().join(format!("{}.jsonl", ids[0])), "").unwrap();

    assert_eq!(page_ids(journal.path(), 0, 10), ids[1..11]);
    assert_eq!(page_ids(journal.path(), 10, 10), ids[11..12]);
}

#[test]
fn each_session_counts_its_carried_out_operations_by_kind_with_their_total_size() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    // One copy vanishes, so its Operation is skipped.
    fs::remove_file(dir.path().join("b/c/copy.txt")).unwrap();
    run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let session = &all_sessions(journal.path())[0];

    assert_eq!(
        session.total_size, 13,
        "only the carried-out Operation counts"
    );
    assert_eq!(session.kind_counts.get(&OperationKind::Trash), Some(&1));
}

#[test]
fn undone_operations_still_count_towards_the_session() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    undo_operations(
        summary.session_id.as_deref().unwrap(),
        &[id_of(&plan, "a/copy.txt")],
        journal.path(),
    )
    .unwrap();

    let session = &all_sessions(journal.path())[0];

    assert_eq!(session.total_size, 26);
    assert_eq!(session.kind_counts.get(&OperationKind::Trash), Some(&2));
    assert_eq!(session.left_to_undo, 1);
}

#[test]
fn sessions_started_in_the_same_millisecond_keep_their_order() {
    let journal = journal_dir();
    let base = "2026-01-01_00-00-00-000Z";
    let ids: Vec<String> = ["", "-002", "-010"]
        .iter()
        .map(|n| format!("{base}{n}"))
        .collect();
    for id in &ids {
        fake_journal(journal.path(), id, 1);
    }
    fake_journal(journal.path(), "2026-01-01_00-00-00-001Z", 1);

    assert_eq!(
        page_ids(journal.path(), 0, 10),
        ["2026-01-01_00-00-00-001Z", &ids[2], &ids[1], &ids[0]]
    );
}

fn append_record(journal: &Path, id: &str, record: serde_json::Value) {
    journal_database(journal)
        .execute(
            "INSERT INTO records (session_id, record) VALUES (?1, ?2)",
            [id, &record.to_string()],
        )
        .unwrap();
}

#[test]
fn an_undo_cut_short_after_the_move_counts_as_restored_not_unrecoverable() {
    let dir = messy_folder();
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    // Undo moved a/copy.txt back, then the app crashed before journaling it.
    let id = id_of(&plan, "a/copy.txt");
    fs::rename(
        landed(
            journal.path(),
            summary.session_id.as_deref().unwrap(),
            "a/copy.txt",
        ),
        dir.path().join("a/copy.txt"),
    )
    .unwrap();
    append_record(
        journal.path(),
        summary.session_id.as_deref().unwrap(),
        serde_json::json!({ "record": "intent", "operationId": id, "phase": "undo" }),
    );

    let undone = undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert!(
        undone.unrecoverable.is_empty(),
        "{:?}",
        undone.unrecoverable
    );
    assert_eq!(undone.done.len(), 1, "only b/c/copy.txt was left to undo");
    assert_eq!(snapshot(dir.path()), before);
    assert_eq!(
        all_sessions(journal.path())[0].status,
        SessionStatus::Undone
    );
}

#[test]
fn an_undo_cut_short_before_the_move_is_simply_undone_again() {
    let dir = messy_folder();
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    let id = id_of(&plan, "a/copy.txt");
    append_record(
        journal.path(),
        summary.session_id.as_deref().unwrap(),
        serde_json::json!({ "record": "intent", "operationId": id, "phase": "undo" }),
    );

    let undone = undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(undone.done.len(), 2);
    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn a_redo_cut_short_after_the_trash_says_to_restore_it_by_hand() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();
    let id = id_of(&plan, "a/copy.txt");
    // The Redo's Trash happened, then the app crashed before recording where.
    fs::remove_file(dir.path().join("a/copy.txt")).unwrap();
    append_record(
        journal.path(),
        summary.session_id.as_deref().unwrap(),
        serde_json::json!({ "record": "intent", "operationId": &id, "phase": "redo" }),
    );

    assert_eq!(
        states(
            journal.path(),
            dir.path(),
            summary.session_id.as_deref().unwrap()
        )[0]
        .1,
        OperationState::Unrecoverable
    );
    let undone = undo_operations(
        summary.session_id.as_deref().unwrap(),
        &[id],
        journal.path(),
    )
    .unwrap();
    assert!(undone.done.is_empty(), "the app never guesses: {undone:?}");
}

#[test]
fn an_item_restored_by_hand_is_no_longer_unrecoverable() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    let session_id = summary.session_id.as_deref().unwrap();
    undo(session_id, journal.path()).unwrap();
    let id = id_of(&plan, "a/copy.txt");
    // The Redo's Trash happened, then the app crashed before recording where.
    fs::remove_file(dir.path().join("a/copy.txt")).unwrap();
    append_record(
        journal.path(),
        session_id,
        serde_json::json!({ "record": "intent", "operationId": &id, "phase": "redo" }),
    );
    let op_of = |details: SessionDetails| {
        details
            .operations
            .into_iter()
            .find(|op| op.id == id)
            .unwrap()
    };
    let lost = op_of(session_details(session_id, journal.path()).unwrap());
    assert_eq!(lost.state, OperationState::Unrecoverable);
    assert!(lost.note.unwrap().contains("by hand"));

    write(dir.path(), "a/copy.txt", "same contents");

    let back = op_of(session_details(session_id, journal.path()).unwrap());
    assert_eq!(back.state, OperationState::Undone);
    assert_eq!(back.note, None);
}

#[test]
fn a_move_cut_short_after_it_happened_can_still_be_undone() {
    let dir = scan_root();
    let journal = journal_dir();
    let plan = talk_into_slides(dir.path());
    let before = snapshot(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    // As if the app crashed right after the move.
    drop_outcomes(journal.path(), summary.session_id.as_deref().unwrap());
    assert_eq!(
        all_sessions(journal.path())[0].status,
        SessionStatus::Crashed
    );

    let undone = undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(undone.done.len(), 1);
    assert_eq!(snapshot(dir.path()), before);
}

fn folder_plan_with_a_blocked_move(root: &Path) -> Plan {
    write(root, "lecture.pdf", "one");
    write(root, "slides.pptx", "two");
    let config = empty_config(root);
    let scan = scan(root, &config).unwrap();
    let folder = SuggestedFolderChoice {
        name: "CS101".to_string(),
        choice: FolderChoice::CreateHere,
        files: ["lecture.pdf", "slides.pptx"]
            .map(|name| (scan.root.join(name), FileChoice::InFolder))
            .to_vec(),
    };
    let mut plan = build_suggested_folders_plan(&scan, &[folder]).unwrap();
    let slides = plan
        .operations
        .iter_mut()
        .find(|op| op.relative_path == ["slides.pptx"])
        .unwrap();
    slides.error = Some("slides.pptx is on a different disk".to_string());
    slides.ticked = false;
    plan
}

#[test]
fn an_approved_blocked_move_is_skipped_with_its_note_and_the_rest_are_done() {
    let dir = scan_root();
    let journal = journal_dir();
    let plan = folder_plan_with_a_blocked_move(dir.path());
    let approved: Vec<String> = plan.operations.iter().map(|op| op.id.clone()).collect();

    let summary = run(&plan, &approved, journal.path()).unwrap();

    assert!(dir.path().join("CS101/lecture.pdf").exists());
    assert!(dir.path().join("slides.pptx").exists());
    assert_eq!(summary.done.len(), 2, "the folder and lecture.pdf");
    assert_eq!(
        summary.skipped[0].note.as_deref(),
        Some("slides.pptx is on a different disk")
    );
    let id = summary.session_id.unwrap();
    assert_eq!(
        states(journal.path(), dir.path(), &id)[2],
        ("slides.pptx".to_string(), OperationState::Skipped)
    );
}

#[test]
fn a_run_where_nothing_happens_keeps_no_session_and_says_why() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    fs::remove_file(dir.path().join("a/copy.txt")).unwrap();
    fs::remove_file(dir.path().join("b/c/copy.txt")).unwrap();

    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    assert_eq!(summary.session_id, None);
    assert!(summary.skipped[0]
        .note
        .as_deref()
        .unwrap()
        .contains("no longer exists"));
    assert!(all_sessions(journal.path()).is_empty());
}

#[test]
fn retrying_a_move_skipped_for_its_missing_folder_does_it_in_the_same_session() {
    let dir = scan_root();
    let journal = journal_dir();
    write(dir.path(), "lecture.pdf", "one");
    write(dir.path(), "invoice.pdf", "two");
    let config = empty_config(dir.path());
    let scan = scan(dir.path(), &config).unwrap();
    let folder = |name: &str, file: &str| SuggestedFolderChoice {
        name: name.to_string(),
        choice: FolderChoice::CreateHere,
        files: vec![(scan.root.join(file), FileChoice::InFolder)],
    };
    let plan = build_suggested_folders_plan(
        &scan,
        &[
            folder("CS101", "lecture.pdf"),
            folder("Bills", "invoice.pdf"),
        ],
    )
    .unwrap();
    // Everything but creating CS101, so the move into it is skipped.
    let approved: Vec<String> = plan
        .operations
        .iter()
        .filter(|op| op.relative_path != ["CS101"])
        .map(|op| op.id.clone())
        .collect();
    let summary = run(&plan, &approved, journal.path()).unwrap();
    let id = summary.session_id.unwrap();
    assert_eq!(summary.skipped.len(), 1);
    fs::create_dir(dir.path().join("CS101")).unwrap();

    let retried = retry_operations(&id, &[id_of(&plan, "lecture.pdf")], journal.path()).unwrap();

    assert_eq!(retried.session_id.as_deref(), Some(id.as_str()));
    assert_eq!(retried.done.len(), 1);
    assert!(dir.path().join("CS101/lecture.pdf").exists());
    assert_eq!(all_sessions(journal.path()).len(), 1);
    assert!(states(journal.path(), dir.path(), &id)
        .iter()
        .all(|(_, state)| *state == OperationState::Done));
    undo(&id, journal.path()).unwrap();
    assert!(dir.path().join("lecture.pdf").exists() && dir.path().join("invoice.pdf").exists());
}

fn talk_into_slides(root: &Path) -> Plan {
    write(root, "talk.pptx", "talk");
    write(root, "Slides/other.txt", "other");
    let slides = FolderChoice::AddTo {
        folder: real_path(&root.join("Slides")),
    };
    common::folder_plan(root, "Slides", slides, &["talk.pptx"])
}

#[test]
fn a_name_taken_since_planning_gets_a_clash_suffix_and_nothing_is_overwritten() {
    let dir = scan_root();
    let journal = journal_dir();
    let plan = talk_into_slides(dir.path());
    write(dir.path(), "Slides/Talk.pptx", "someone else's");

    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();

    assert_eq!(summary.done.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("Slides/Talk.pptx")).unwrap(),
        "someone else's"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("Slides/talk (1).pptx")).unwrap(),
        "talk"
    );
    undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("talk.pptx")).unwrap(),
        "talk"
    );
    assert!(dir.path().join("Slides/Talk.pptx").exists());
}

#[test]
fn undo_leaves_a_file_the_user_moved_again_where_they_put_it() {
    let dir = scan_root();
    let journal = journal_dir();
    let plan = talk_into_slides(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    write(dir.path(), "Elsewhere/keep.txt", "");
    fs::rename(
        dir.path().join("Slides/talk.pptx"),
        dir.path().join("Elsewhere/talk.pptx"),
    )
    .unwrap();

    let undone = undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(undone.unrecoverable.len(), 1);
    assert!(undone.unrecoverable[0]
        .note
        .as_deref()
        .unwrap()
        .contains("is gone"));
    assert!(dir.path().join("Elsewhere/talk.pptx").exists());
    assert!(!dir.path().join("talk.pptx").exists());
}

fn record_count(journal: &Path, id: &str) -> i64 {
    journal_database(journal)
        .query_row(
            "SELECT count(*) FROM records WHERE session_id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap()
}

/// As an older version left it.
fn as_old_journal_file(journal: &Path, id: &str) {
    let db = journal_database(journal);
    let (root, started_at): (String, i64) = db
        .query_row(
            "SELECT scan_root, started_at FROM sessions WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let header = serde_json::json!({ "id": id, "scanRoot": root, "startedAt": started_at });
    let mut lines = format!("{header}\n");
    let mut query = db
        .prepare("SELECT record FROM records WHERE session_id = ?1 ORDER BY seq")
        .unwrap();
    for record in query
        .query_map([id], |row| row.get::<_, String>(0))
        .unwrap()
    {
        lines.push_str(&record.unwrap());
        lines.push('\n');
    }
    drop(query);
    drop(db);
    for name in ["journal.sqlite", "journal.sqlite-wal", "journal.sqlite-shm"] {
        let _ = fs::remove_file(journal.join(name));
    }
    fs::write(journal.join(format!("{id}.jsonl")), lines).unwrap();
}

#[test]
fn a_session_journaled_before_the_database_is_imported_and_can_still_be_undone() {
    let dir = messy_folder();
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let id = run(&plan, &all_ids(&plan), journal.path())
        .unwrap()
        .session_id
        .unwrap();
    as_old_journal_file(journal.path(), &id);

    let undone = undo(&id, journal.path()).unwrap();

    assert_eq!(undone.done.len(), 2);
    assert_eq!(snapshot(dir.path()), before);
    assert!(!journal.path().join(format!("{id}.jsonl")).exists());
    assert_eq!(
        all_sessions(journal.path())[0].status,
        SessionStatus::Undone
    );
}

#[test]
fn an_old_journal_found_again_after_its_import_isnt_imported_twice() {
    let journal = journal_dir();
    let id = "2026-01-01_00-00-00-000Z";
    fake_journal(journal.path(), id, 4);
    assert_eq!(all_sessions(journal.path()).len(), 1);
    // A crash between the import's commit and deleting the file.
    fake_journal(journal.path(), id, 4);

    assert_eq!(all_sessions(journal.path()).len(), 1);
    assert_eq!(record_count(journal.path(), id), 3);
    assert!(!journal.path().join(format!("{id}.jsonl")).exists());
}

#[test]
fn an_old_journal_cut_short_mid_record_is_imported_without_that_record() {
    let journal = journal_dir();
    let id = "2026-01-01_00-00-00-000Z";
    fake_journal(journal.path(), id, 4);
    let path = journal.path().join(format!("{id}.jsonl"));
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str(r#"{"record":"intent","operationId":"#);
    fs::write(&path, text).unwrap();

    assert_eq!(page_ids(journal.path(), 0, 10), [id]);
    assert_eq!(record_count(journal.path(), id), 3);
    assert!(!path.exists());
}

#[test]
fn an_old_journal_damaged_before_its_last_line_is_left_where_it_is() {
    let journal = journal_dir();
    let id = "2026-01-01_00-00-00-000Z";
    let path = journal.path().join(format!("{id}.jsonl"));
    let header = serde_json::json!({ "id": id, "scanRoot": "/root", "startedAt": 0 });
    fs::write(
        &path,
        format!("{header}\nnot a record\n{{\"record\":\"finished\"}}\n"),
    )
    .unwrap();

    assert!(all_sessions(journal.path()).is_empty());
    assert!(path.exists());
}

#[test]
fn a_record_from_a_newer_version_hides_only_its_own_session() {
    let journal = journal_dir();
    let mut ids = fake_journals(journal.path(), 3);
    ids.reverse();
    assert_eq!(page_ids(journal.path(), 0, 10), ids);

    append_record(
        journal.path(),
        &ids[1],
        serde_json::json!({ "record": "fromTheFuture" }),
    );

    assert_eq!(
        page_ids(journal.path(), 0, 10),
        [ids[0].clone(), ids[2].clone()]
    );
    assert!(matches!(
        session_details(&ids[1], journal.path()),
        Err(SessionError::BadJournal { .. })
    ));
}

#[test]
fn undo_passes_over_a_newer_session_it_cant_read() {
    let dir = messy_folder();
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let id = run(&plan, &all_ids(&plan), journal.path())
        .unwrap()
        .session_id
        .unwrap();
    let newer = "2999-01-01_00-00-00-000Z";
    fake_journal(journal.path(), newer, 4);
    all_sessions(journal.path());
    append_record(
        journal.path(),
        newer,
        serde_json::json!({ "record": "fromTheFuture" }),
    );

    undo(&id, journal.path()).unwrap();

    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn listing_the_sessions_of_a_folder_never_used_creates_nothing() {
    let journal = journal_dir();
    let missing = journal.path().join("Sessions");

    assert_eq!(list_sessions(&missing, 0, 10).unwrap().total, 0);

    assert!(!missing.exists());
}

#[test]
fn sessions_can_be_listed_and_read_while_a_run_writes_its_journal() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    let mut seen = Vec::new();

    let summary = run_with_progress(&plan, &all_ids(&plan), journal.path(), |_| {
        let page = list_sessions(journal.path(), 0, 10).unwrap();
        session_details(&page.sessions[0].id, journal.path()).unwrap();
        seen.push(page.sessions[0].id.clone());
    })
    .unwrap();

    assert!(!seen.is_empty());
    assert!(seen
        .iter()
        .all(|id| Some(id) == summary.session_id.as_ref()));
}

#[test]
fn a_session_of_files_with_accents_quotes_and_emoji_in_their_names_undoes() {
    let dir = scan_root();
    for (path, secs) in [
        ("giữ lại.txt", 3_000),
        ("Tài liệu/bản sao 'cũ' 😀 100%.txt", 2_000),
    ] {
        set_modified(&write(dir.path(), path, "same contents"), secs);
    }
    let journal = journal_dir();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &all_ids(&plan), journal.path()).unwrap();
    assert_eq!(summary.done.len(), 1);

    undo(summary.session_id.as_deref().unwrap(), journal.path()).unwrap();

    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn a_run_where_nothing_happens_leaves_no_records_behind() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    fs::remove_file(dir.path().join("a/copy.txt")).unwrap();
    fs::remove_file(dir.path().join("b/c/copy.txt")).unwrap();

    run(&plan, &all_ids(&plan), journal.path()).unwrap();

    let records: i64 = journal_database(journal.path())
        .query_row("SELECT count(*) FROM records", [], |row| row.get(0))
        .unwrap();
    assert_eq!(records, 0);
}

#[test]
fn deleting_every_session_leaves_no_records_and_the_next_run_starts_afresh() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    run(&plan, &all_ids(&plan), journal.path()).unwrap();

    assert_eq!(delete_all_sessions(journal.path()).unwrap(), 1);

    let records: i64 = journal_database(journal.path())
        .query_row("SELECT count(*) FROM records", [], |row| row.get(0))
        .unwrap();
    assert_eq!(records, 0);
    let dir = messy_folder();
    let plan = plan_for(dir.path());
    run(&plan, &all_ids(&plan), journal.path()).unwrap();
    assert_eq!(all_sessions(journal.path()).len(), 1);
}

#[test]
fn deleting_every_session_starts_a_journal_damaged_past_its_first_page_again() {
    let dir = messy_folder();
    let journal = journal_dir();
    let plan = plan_for(dir.path());
    run(&plan, &all_ids(&plan), journal.path()).unwrap();
    journal_database(journal.path())
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    // Opening it reads only the first page, so the damage goes unseen until then.
    let path = journal.path().join("journal.sqlite");
    let mut bytes = fs::read(&path).unwrap();
    bytes[4096..].fill(0xAB);
    fs::write(&path, bytes).unwrap();

    delete_all_sessions(journal.path()).unwrap();

    let dir = messy_folder();
    let plan = plan_for(dir.path());
    run(&plan, &all_ids(&plan), journal.path()).unwrap();
    assert_eq!(all_sessions(journal.path()).len(), 1);
}
