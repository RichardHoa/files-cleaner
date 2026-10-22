//! The safety edges of a Plan: folders it empties, Operations it can't
//! carry out, and undoing an older Session after a newer one.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use common::{config, empty_config, scan_root, write};
use files_cleaner_core::{
    build_plan, build_rule_plan, list_sessions, run, scan, session_details, undo, undo_operations,
    OperationKind, OperationState, Plan, Reason, SessionStatus, HOLDING_FOLDER_NAME,
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

/// Writes a copy of the Keeper `keep.txt` at each of `copies`, all older.
fn with_copies(root: &Path, copies: &[&str]) {
    set_modified(&write(root, "keep.txt", "same contents"), 9_000);
    for (i, copy) in copies.iter().enumerate() {
        set_modified(&write(root, copy, "same contents"), 1_000 + i as u64);
    }
}

fn plan_for(root: &Path) -> Plan {
    let config = empty_config(root);
    build_plan(&scan(root, &config).unwrap(), &config)
}

/// The folders the Plan offers to trash because it empties them, as
/// relative paths.
fn emptied(plan: &Plan) -> Vec<String> {
    plan.operations
        .iter()
        .filter(|op| op.reason == Reason::EmptiedFolder)
        .map(|op| op.relative_path.join("/"))
        .collect()
}

fn ticked(plan: &Plan) -> Vec<String> {
    plan.operations
        .iter()
        .filter(|op| op.ticked)
        .map(|op| op.id.clone())
        .collect()
}

fn every_id(plan: &Plan) -> Vec<String> {
    plan.operations.iter().map(|op| op.id.clone()).collect()
}

/// Every file and folder under `root`, `/`-separated, with each file's
/// contents.
fn snapshot(root: &Path) -> BTreeMap<String, Option<String>> {
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
                found.insert(relative, Some(fs::read_to_string(&path).unwrap()));
            }
        }
    }
    found
}

#[test]
fn a_folder_the_plan_empties_is_offered_for_trash_unticked() {
    let dir = scan_root();
    with_copies(
        dir.path(),
        &["old/copy.txt", "old/deeper/copy 2.txt", "kept/copy.txt"],
    );
    write(dir.path(), "kept/notes.pdf", "stays");

    let plan = plan_for(dir.path());

    assert_eq!(
        emptied(&plan),
        ["old"],
        "only the topmost emptied folder, and not one that keeps a file"
    );
    let op = plan
        .operations
        .iter()
        .find(|op| op.reason == Reason::EmptiedFolder)
        .unwrap();
    assert_eq!(op.kind, OperationKind::Trash);
    assert!(!op.ticked);
    assert_eq!(op.error, None);
    assert_eq!(
        op.destination.join("/"),
        "Files Cleaner Trash/This Session/old"
    );
    assert_eq!(
        plan.operations.last().unwrap().id,
        op.id,
        "it runs after the files leave"
    );
}

#[test]
fn os_metadata_files_dont_keep_a_folder_but_anything_else_does() {
    let dir = scan_root();
    with_copies(
        dir.path(),
        &[
            "metadata/copy.txt",
            "hidden/copy.txt",
            "protected/copy.txt",
            "noext/copy.txt",
            "nested/a/b/copy.txt",
        ],
    );
    write(dir.path(), "metadata/.DS_Store", "ds");
    write(dir.path(), "metadata/Thumbs.db", "thumbs");
    write(dir.path(), "metadata/sub/desktop.ini", "ini");
    write(dir.path(), "hidden/.gitkeep", "");
    write(dir.path(), "protected/node_modules/x/index.js", "js");
    write(dir.path(), "noext/Makefile", "all:");
    fs::create_dir_all(dir.path().join("nested/empty already")).unwrap();

    let plan = plan_for(dir.path());

    assert_eq!(emptied(&plan), ["metadata", "nested"]);
}

#[test]
fn a_folder_a_rule_moves_files_into_is_not_emptied() {
    let dir = scan_root();
    write(dir.path(), "Slides/old.key.pdf", "x");
    write(dir.path(), "inbox/talk.pptx", "talk");
    let config = config(
        r#"{ "rules": [
            { "name": "Slides", "match": { "extensions": ["pptx"] }, "then": { "moveTo": "inbox/sorted" } },
            { "name": "PDFs", "match": { "extensions": ["pdf"] }, "then": { "moveTo": "Papers" } }
        ] }"#,
        dir.path(),
    );

    let plan = build_rule_plan(&scan(dir.path(), &config).unwrap(), &config);

    assert_eq!(
        emptied(&plan),
        ["Slides"],
        "inbox gets the sorted folder, so it isn't emptied"
    );
}

#[test]
fn trashing_an_emptied_folder_takes_its_metadata_along_and_undo_restores_it_exactly() {
    let dir = scan_root();
    with_copies(dir.path(), &["old/copy.txt", "old/deeper/copy 2.txt"]);
    write(dir.path(), "old/.DS_Store", "ds");
    let journal = TempDir::new().unwrap();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());

    let summary = run(&plan, &every_id(&plan), journal.path()).unwrap();

    assert_eq!(summary.done.len(), 3, "{summary:?}");
    assert!(!dir.path().join("old").exists());
    let holding = dir
        .path()
        .join(HOLDING_FOLDER_NAME)
        .join(&summary.session_id);
    assert_eq!(
        fs::read_to_string(holding.join("old/copy.txt")).unwrap(),
        "same contents"
    );
    // The copies' folder took the name first, so the emptied folder gets a Clash Suffix.
    assert_eq!(
        fs::read_to_string(holding.join("old (1)/.DS_Store")).unwrap(),
        "ds"
    );

    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(undone.done.len(), 3, "{undone:?}");
    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn an_emptied_folder_with_something_new_in_it_is_skipped_at_run_time() {
    let dir = scan_root();
    with_copies(dir.path(), &["old/copy.txt", "unticked/copy.txt"]);
    let journal = TempDir::new().unwrap();
    let plan = plan_for(dir.path());
    write(dir.path(), "old/sub/new download.zip", "new");
    let skip_copy = plan
        .operations
        .iter()
        .find(|op| op.relative_path.join("/") == "unticked/copy.txt")
        .unwrap();
    let approved: Vec<String> = every_id(&plan)
        .into_iter()
        .filter(|id| *id != skip_copy.id)
        .collect();

    let summary = run(&plan, &approved, journal.path()).unwrap();

    let notes: BTreeMap<String, String> = summary
        .skipped
        .iter()
        .map(|o| {
            (
                o.source.file_name().unwrap().to_string_lossy().into_owned(),
                o.note.clone().unwrap(),
            )
        })
        .collect();
    assert_eq!(notes.len(), 2, "{summary:?}");
    assert!(
        notes["old"].contains("isn't empty") && notes["old"].contains("new download.zip"),
        "{notes:?}"
    );
    assert!(notes["unticked"].contains("copy.txt"), "{notes:?}");
    assert!(dir.path().join("old/sub/new download.zip").exists());
    assert!(dir.path().join("unticked/copy.txt").exists());
}

#[test]
fn undoing_the_files_first_then_the_emptied_folder_merges_it_back() {
    let dir = scan_root();
    with_copies(dir.path(), &["old/copy.txt"]);
    write(dir.path(), "old/Thumbs.db", "thumbs");
    write(dir.path(), "old/empty/desktop.ini", "ini");
    let journal = TempDir::new().unwrap();
    let before = snapshot(dir.path());
    let plan = plan_for(dir.path());
    let summary = run(&plan, &every_id(&plan), journal.path()).unwrap();
    let copy = plan
        .operations
        .iter()
        .find(|op| op.relative_path.join("/") == "old/copy.txt")
        .unwrap();

    // Restoring the file alone makes `old` again.
    undo_operations(&summary.session_id, &[copy.id.clone()], journal.path()).unwrap();
    let undone = undo(&summary.session_id, journal.path()).unwrap();

    assert_eq!(undone.done.len(), 1, "{undone:?}");
    assert_eq!(snapshot(dir.path()), before);
}

/// Mounts a fresh empty disk (tmpfs) at `folder` and unmounts it when
/// dropped. `None` if that isn't allowed (it needs root).
#[cfg(target_os = "linux")]
struct OtherDisk(std::path::PathBuf);

#[cfg(target_os = "linux")]
impl OtherDisk {
    fn mount(folder: &Path) -> Option<OtherDisk> {
        fs::create_dir_all(folder).unwrap();
        let mounted = std::process::Command::new("mount")
            .args(["-t", "tmpfs", "-o", "size=1m", "tmpfs"])
            .arg(folder)
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        mounted.then(|| OtherDisk(folder.to_path_buf()))
    }
}

#[cfg(target_os = "linux")]
impl Drop for OtherDisk {
    fn drop(&mut self) {
        let _ = std::process::Command::new("umount").arg(&self.0).status();
    }
}

#[test]
#[cfg(target_os = "linux")]
fn moves_and_trash_across_disks_are_errors_that_are_never_run() {
    let dir = scan_root();
    let Some(_disk) = OtherDisk::mount(&dir.path().join("usb")) else {
        eprintln!("skipped: mounting a second disk needs root");
        return;
    };
    with_copies(dir.path(), &["usb/copy.txt", "here/copy.txt"]);
    write(dir.path(), "usb/talk.pptx", "talk");
    let config = config(
        r#"{ "rules": [{ "name": "Slides", "match": { "extensions": ["pptx"] }, "then": { "moveTo": "Slides" } }] }"#,
        dir.path(),
    );
    let scanned = scan(dir.path(), &config).unwrap();
    let errors = |plan: &Plan| -> BTreeMap<String, String> {
        plan.operations
            .iter()
            .filter_map(|op| Some((op.relative_path.join("/"), op.error.clone()?)))
            .collect()
    };

    let moves = errors(&build_rule_plan(&scanned, &config));
    assert_eq!(moves.keys().collect::<Vec<_>>(), ["usb/talk.pptx"]);
    assert!(
        moves.values().all(|e| e.contains("different disk")),
        "{moves:?}"
    );

    let plan = build_plan(&scanned, &config);
    let trashes = errors(&plan);
    assert_eq!(trashes.keys().collect::<Vec<_>>(), ["usb/copy.txt"]);
    assert!(
        trashes.values().all(|e| e.contains("different disk")),
        "{trashes:?}"
    );
    let blocked = plan.operations.iter().filter(|op| op.error.is_some());
    assert!(blocked.clone().all(|op| !op.ticked));
    assert_eq!(
        emptied(&plan),
        ["here"],
        "a folder whose items can't leave isn't emptied"
    );

    let journal = TempDir::new().unwrap();
    let summary = run(&plan, &every_id(&plan), journal.path()).unwrap();

    let ran: Vec<String> = summary
        .done
        .iter()
        .chain(&summary.skipped)
        .chain(&summary.failed)
        .map(|o| o.operation_id.clone())
        .collect();
    assert!(blocked.clone().all(|op| !ran.contains(&op.id)), "{ran:?}");
    assert!(dir.path().join("usb/copy.txt").exists() && dir.path().join("usb/talk.pptx").exists());
    assert!(
        !dir.path().join("here").exists(),
        "the rest of the Plan runs"
    );
}

fn rules(text: &str, root: &Path) -> Plan {
    let config = config(&format!(r#"{{ "rules": [{text}] }}"#), root);
    build_rule_plan(&scan(root, &config).unwrap(), &config)
}

#[test]
fn undoing_an_older_session_skips_what_a_newer_one_moved_on_and_undoes_the_rest() {
    let dir = scan_root();
    write(dir.path(), "talk.pptx", "talk");
    write(dir.path(), "intro.pptx", "intro");
    let journal = TempDir::new().unwrap();
    let before = snapshot(dir.path());
    let to_study = r#"{ "name": "Slides", "match": { "extensions": ["pptx"] }, "then": { "moveTo": "Study" } }"#;
    let plan = rules(to_study, dir.path());
    let older = run(&plan, &ticked(&plan), journal.path()).unwrap();
    let plan = rules(
        r#"{ "name": "Old talks", "match": { "nameGlob": "talk*" }, "then": { "trash": true } }"#,
        dir.path(),
    );
    let newer = run(&plan, &ticked(&plan), journal.path()).unwrap();
    assert_eq!(
        newer.done.len(),
        1,
        "the newer Session trashed Study/talk.pptx"
    );

    let undone = undo(&older.session_id, journal.path()).unwrap();

    assert_eq!(
        fs::read_to_string(dir.path().join("intro.pptx")).unwrap(),
        "intro"
    );
    let notes: BTreeMap<String, String> = undone
        .skipped
        .iter()
        .map(|o| {
            (
                o.source.file_name().unwrap().to_string_lossy().into_owned(),
                o.note.clone().unwrap(),
            )
        })
        .collect();
    assert_eq!(notes.len(), 2, "{undone:?}");
    assert!(
        notes["talk.pptx"].contains(&newer.session_id)
            && notes["talk.pptx"].contains("undo that Session first")
    );
    assert!(
        notes["Study"].contains("aren't back yet"),
        "the folder waits for talk.pptx: {notes:?}"
    );
    let details = session_details(&older.session_id, journal.path()).unwrap();
    assert_eq!(details.info.status, SessionStatus::PartlyUndone);
    let talk = details
        .operations
        .iter()
        .find(|op| op.source.ends_with("talk.pptx"))
        .unwrap();
    assert_eq!(talk.state, OperationState::Done);

    // Once the newer Session is undone, the older one can finish.
    undo(&newer.session_id, journal.path()).unwrap();
    let undone = undo(&older.session_id, journal.path()).unwrap();

    assert_eq!(
        undone.done.len(),
        2,
        "talk.pptx, then its folder: {undone:?}"
    );
    assert_eq!(snapshot(dir.path()), before);
    let statuses: Vec<SessionStatus> = list_sessions(journal.path(), 0, 10)
        .unwrap()
        .sessions
        .iter()
        .map(|s| s.status)
        .collect();
    assert_eq!(statuses, [SessionStatus::Undone, SessionStatus::Undone]);
}

#[test]
fn undoing_an_older_session_skips_a_place_a_newer_one_filled() {
    let dir = scan_root();
    write(dir.path(), "inbox/report.pdf", "first");
    let journal = TempDir::new().unwrap();
    let to_papers = r#"{ "name": "Papers", "match": { "extensions": ["pdf"] }, "then": { "moveTo": "Papers" } }"#;
    let plan = rules(to_papers, dir.path());
    let older = run(&plan, &ticked(&plan), journal.path()).unwrap();
    // A newer Session puts a new download where the old one was.
    write(dir.path(), "downloads/new-report.pdf", "second");
    let to_inbox = r#"{ "name": "Inbox", "match": { "nameGlob": "new-*" }, "then": { "moveTo": "inbox", "rename": "report" } }"#;
    let plan = rules(to_inbox, dir.path());
    let newer = run(&plan, &ticked(&plan), journal.path()).unwrap();
    assert!(
        newer
            .done
            .iter()
            .any(|o| o.destination.ends_with("inbox/report.pdf")),
        "{newer:?}"
    );

    let undone = undo(&older.session_id, journal.path()).unwrap();

    let note = undone.skipped.iter().find_map(|o| o.note.clone()).unwrap();
    assert!(
        note.contains(&newer.session_id) && note.contains("put another item"),
        "{note}"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("inbox/report.pdf")).unwrap(),
        "second"
    );
    assert!(undone.unrecoverable.is_empty());
}
