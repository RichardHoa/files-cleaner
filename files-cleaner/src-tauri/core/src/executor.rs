//! Changes the disk, journaling every step before it happens: runs the
//! approved Operations of a Plan as a new Session, and undoes or redoes the
//! Operations of a past one.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::journal::{lock_sessions, Journal, OutcomeKind, Phase, Record, SessionError};
use crate::planner::{OperationKind, Plan, SourceState};
use crate::protection::HOLDING_FOLDER_NAME;
use crate::scanner::{millis, total_size};
use crate::sessions::{JournaledOperation, OperationState, SessionJournal};

/// What happened to each Operation in one Run, Undo or Redo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub session_id: String,
    /// Moved: trashed by a Run or a Redo, restored by an Undo.
    pub done: Vec<Outcome>,
    /// Not moved, but could be later: the item changed, or something is in
    /// the way.
    pub skipped: Vec<Outcome>,
    /// Run only: the move failed.
    pub failed: Vec<Outcome>,
    /// Undo only: the trashed copy is gone, so it never can be undone.
    pub unrecoverable: Vec<Outcome>,
}

impl Summary {
    fn new(session_id: String) -> Self {
        Summary { session_id, done: Vec::new(), skipped: Vec::new(), failed: Vec::new(), unrecoverable: Vec::new() }
    }

    fn push(&mut self, kind: OutcomeKind, outcome: Outcome) {
        match kind {
            OutcomeKind::Done => self.done.push(outcome),
            OutcomeKind::Skipped => self.skipped.push(outcome),
            OutcomeKind::Failed => self.failed.push(outcome),
            OutcomeKind::Unrecoverable => self.unrecoverable.push(outcome),
        }
    }
}

/// One Operation's outcome, in a Run, an Undo or a Redo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub operation_id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    pub destination: PathBuf,
    /// Why it wasn't done.
    pub note: Option<String>,
}

/// Why an Operation's pass didn't move the item.
type NotDone = (OutcomeKind, String);

/// Runs the Operations of `plan` whose IDs are in `approved`, in Plan
/// order, as a new Session journaled in `journal_dir`. Trashed items go to
/// `<Scan Root>/Files Cleaner Trash/<Session ID>/<original relative path>`.
pub fn run(plan: &Plan, approved: &[String], journal_dir: &Path) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let approved: HashSet<&str> = approved.iter().map(String::as_str).collect();
    let (header, mut journal) = Journal::create(journal_dir, plan.root.clone(), plan.trash_destination)?;

    let holding = plan.root.join(HOLDING_FOLDER_NAME).join(&header.id);
    let operations: Vec<JournaledOperation> = plan
        .operations
        .iter()
        .filter(|op| approved.contains(op.id.as_str()))
        .map(|op| JournaledOperation {
            id: op.id.clone(),
            kind: op.kind,
            source: op.source.clone(),
            destination: holding.join(op.relative_path.iter().collect::<PathBuf>()),
            planned: op.planned,
            state: OperationState::Started,
            note: None,
            interrupted: None,
        })
        .collect();

    let mut summary = Summary::new(header.id);
    apply_each(&mut journal, &mut summary, Phase::Run, operations.iter(), |op| {
        unchanged(&op.source, &op.planned, "since the Plan was made").map_err(|r| (OutcomeKind::Skipped, r))?;
        move_without_overwriting(&op.source, &op.destination).map_err(|r| (OutcomeKind::Failed, r))
    })?;
    journal.append(&Record::Finished)?;
    Ok(summary)
}

/// Reverses every done Operation of Session `session_id`, newest first,
/// using only its Journal in `journal_dir`. Never overwrites: an Operation
/// whose original location is taken is skipped, and can be retried by
/// undoing again.
pub fn undo(session_id: &str, journal_dir: &Path) -> Result<Summary, SessionError> {
    undo_selected(session_id, None, journal_dir)
}

/// Like `undo`, for only the Operations in `operation_ids`. Operations that
/// aren't done (say, already undone) are left alone.
pub fn undo_operations(session_id: &str, operation_ids: &[String], journal_dir: &Path) -> Result<Summary, SessionError> {
    let only: HashSet<&str> = operation_ids.iter().map(String::as_str).collect();
    undo_selected(session_id, Some(&only), journal_dir)
}

fn undo_selected(session_id: &str, only: Option<&HashSet<&str>>, journal_dir: &Path) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let (session, mut journal) = reopen(journal_dir, session_id)?;

    let mut summary = Summary::new(session.header.id.clone());
    apply_each(&mut journal, &mut summary, Phase::Undo, session.in_state(OperationState::Done, only).rev(), |op| {
        if !exists(&op.destination) {
            return Err((OutcomeKind::Unrecoverable, format!("The trashed copy at {} is gone", op.destination.display())));
        }
        move_without_overwriting(&op.destination, &op.source).map_err(|note| {
            (OutcomeKind::Skipped, format!("{note}; the trashed copy was left at {}", op.destination.display()))
        })?;
        tidy_after_undo(&session, op);
        Ok(())
    })?;
    Ok(summary)
}

/// Carries out again the undone Operations of Session `session_id` that
/// are in `operation_ids`, in the order they first ran, putting each item
/// back where the Session put it. An item that changed since Undo put it
/// back is skipped, as is one whose destination is taken; Operations that
/// aren't undone are left alone.
pub fn redo_operations(session_id: &str, operation_ids: &[String], journal_dir: &Path) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let (session, mut journal) = reopen(journal_dir, session_id)?;
    let only: HashSet<&str> = operation_ids.iter().map(String::as_str).collect();

    let mut summary = Summary::new(session.header.id.clone());
    apply_each(&mut journal, &mut summary, Phase::Redo, session.in_state(OperationState::Undone, Some(&only)), |op| {
        unchanged(&op.source, &op.planned, "since Undo put it back")
            .and_then(|()| move_without_overwriting(&op.source, &op.destination))
            .map_err(|r| (OutcomeKind::Skipped, r))
    })?;
    Ok(summary)
}

/// Session `session_id`'s Journal, read and open to add to, with every
/// pass a crash cut short settled first.
fn reopen(journal_dir: &Path, session_id: &str) -> Result<(SessionJournal, Journal), SessionError> {
    let mut session = SessionJournal::read(journal_dir, session_id)?;
    let mut journal = Journal::reopen(journal_dir, &session.header.id)?;
    settle_interrupted(&mut session, &mut journal)?;
    Ok((session, journal))
}

/// A crash between a move and its outcome record leaves an intent with no
/// outcome. If the item is where that pass would put it, the move happened:
/// journal it as done (tidying up after an Undo, as it would have), so the
/// Operation is in the state the disk is in. Otherwise the move never
/// happened, and the Operation is as it was.
fn settle_interrupted(session: &mut SessionJournal, journal: &mut Journal) -> io::Result<()> {
    let mut settled = Vec::new();
    for op in &session.operations {
        let Some(phase) = op.interrupted else { continue };
        let (from, to) = op.route(phase);
        if exists(from) || !exists(to) {
            continue;
        }
        if phase == Phase::Undo {
            tidy_after_undo(session, op);
        }
        settled.push(Record::Outcome { operation_id: op.id.clone(), phase, outcome: OutcomeKind::Done, reason: None });
    }
    for record in &settled {
        journal.append(record)?;
        session.apply(record);
    }
    Ok(())
}

/// Removes the folders in the Holding Folder that Undo of `op` left empty.
fn tidy_after_undo(session: &SessionJournal, op: &JournaledOperation) {
    remove_empty_folders(op.destination.parent(), &session.header.scan_root.join(HOLDING_FOLDER_NAME));
}

/// For each of `operations`: journals `phase`'s intent, tries `attempt`,
/// journals how it ended, and adds that to `summary`.
fn apply_each<'a>(
    journal: &mut Journal,
    summary: &mut Summary,
    phase: Phase,
    operations: impl Iterator<Item = &'a JournaledOperation>,
    mut attempt: impl FnMut(&JournaledOperation) -> Result<(), NotDone>,
) -> io::Result<()> {
    for op in operations {
        journal.append(&op.intent(phase))?;
        let (outcome, note) = match attempt(op) {
            Ok(()) => (OutcomeKind::Done, None),
            Err((outcome, note)) => (outcome, Some(note)),
        };
        journal.append(&Record::Outcome { operation_id: op.id.clone(), phase, outcome, reason: note.clone() })?;
        summary.push(outcome, op.outcome(note));
    }
    Ok(())
}

fn exists(path: &Path) -> bool {
    !matches!(fs::symlink_metadata(path), Err(e) if e.kind() == io::ErrorKind::NotFound)
}

/// Fails with a note if `path` is gone or isn't the item `expected`
/// describes: a different kind, size or modified date. `since` ends the
/// note ("since the Plan was made").
fn unchanged(path: &Path, expected: &SourceState, since: &str) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err("It no longer exists".into()),
        Err(e) => return Err(format!("It couldn't be read: {e}")),
    };
    let is_package = metadata.is_dir();
    if metadata.is_symlink() || is_package != expected.is_package {
        return Err(format!("It was replaced by something else {since}"));
    }
    let size = if is_package { total_size(path) } else { metadata.len() };
    if size != expected.size {
        return Err(format!("Its size changed {since}"));
    }
    match metadata.modified() {
        Ok(modified) if millis(modified) == expected.modified => Ok(()),
        _ => Err(format!("It was modified {since}")),
    }
}

/// Moves `from` to `to` with a same-disk rename, creating `to`'s folders.
/// Never replaces anything already at `to`, and never copies.
fn move_without_overwriting(from: &Path, to: &Path) -> Result<(), String> {
    if fs::symlink_metadata(to).is_ok() {
        return Err(format!("{} already exists", to.display()));
    }
    let moved = to.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| fs::rename(from, to));
    moved.map_err(|e| match e.kind() {
        io::ErrorKind::CrossesDevices => format!("{} is on a different disk from {}", from.display(), to.display()),
        _ => format!("Couldn't move it to {}: {e}", to.display()),
    })
}

/// Removes `folder` and the folders above it, up to and including
/// `holding`, while they are empty. Only ever removes empty folders, so
/// nothing in them can be lost.
fn remove_empty_folders(folder: Option<&Path>, holding: &Path) {
    let mut folder = folder;
    while let Some(path) = folder.filter(|path| path.starts_with(holding)) {
        match fs::remove_dir(path) {
            Ok(()) => folder = path.parent(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => folder = path.parent(),
            Err(_) => return,
        }
    }
}
