//! Changes the disk, journaling every step before it happens: runs the
//! approved Operations of a Plan as a new Session, and undoes or redoes the
//! Operations of a past one.

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::clash;
use crate::journal::{self, lock_sessions, Journal, OutcomeKind, Phase, Record, SessionError};
use crate::os_trash::{OsTrash, TRASH_PLACE};
use crate::planner::{Operation, OperationKind, Plan, SourceState};
use crate::protection::is_os_metadata_file;
use crate::scanner::{millis, total_size};
use crate::sessions::{restore_by_hand_note, JournaledOperation, OperationState, SessionJournal};

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
    /// The trashed copy is gone, or where it landed isn't known, so it never
    /// can be undone.
    pub unrecoverable: Vec<Outcome>,
}

impl Summary {
    fn new(session_id: String) -> Self {
        Summary {
            session_id,
            done: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
            unrecoverable: Vec::new(),
        }
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
    /// `None` for a Trash that hasn't landed.
    pub destination: Option<PathBuf>,
    /// Why it wasn't done.
    pub note: Option<String>,
}

/// Why an Operation's pass didn't move the item.
type NotDone = (OutcomeKind, String);

/// Runs the Operations of `plan` whose IDs are in `approved`, in Plan
/// order, as a new Session journaled in `journal_dir`. Trashed items go to
/// `trash`, and where each landed is journaled with its outcome; one that
/// can't go stays in place, skipped with a Note. A Move whose destination
/// was taken since planning gets a new Clash Suffix, worked out just before
/// it is journaled. An Operation the Plan marks as an error is never run,
/// even if approved: a Trash is journaled as skipped, so it is listed.
pub fn run(
    plan: &Plan,
    approved: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    run_with_progress(plan, approved, journal_dir, trash, |_| {})
}

/// How far a Run has got, reported while it runs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProgress {
    /// Operations finished so far, whatever their outcome.
    pub finished: usize,
    /// Operations the Run will carry out in all.
    pub total: usize,
    /// The item the Run is on.
    pub current: PathBuf,
}

/// Like `run`, calling `on_progress` before each Operation (at most every
/// `PROGRESS_INTERVAL` or so), and once more at the end.
pub fn run_with_progress(
    plan: &Plan,
    approved: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
    on_progress: impl FnMut(RunProgress),
) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let approved: HashSet<&str> = approved.iter().map(String::as_str).collect();
    let runs = |op: &&Operation| {
        approved.contains(op.id.as_str()) && (op.error.is_none() || op.kind == OperationKind::Trash)
    };
    let total = plan.operations.iter().filter(runs).count();
    let (header, mut journal) = Journal::create(journal_dir, plan.root.clone())?;
    let blocked: HashMap<&str, &str> = plan
        .operations
        .iter()
        .filter_map(|op| Some((op.id.as_str(), op.error.as_deref()?)))
        .collect();

    // Lazy, so each destination is checked once the Operations before it
    // have run.
    let operations = plan
        .operations
        .iter()
        .filter(runs)
        .map(|op| JournaledOperation {
            id: op.id.clone(),
            kind: op.kind,
            source: op.source.clone(),
            destination: match op.kind {
                OperationKind::Move => op.destination_path.as_deref().map(free_destination),
                OperationKind::Trash | OperationKind::CreateFolder => op.destination_path.clone(),
            },
            in_system_trash: false,
            planned: op.planned,
            state: OperationState::Started,
            note: None,
            interrupted: None,
        });

    let mut summary = Summary::new(header.id);
    let mut progress = Progress::new(total, on_progress);
    let operations = operations.inspect(|op| progress.starting(&op.source));
    apply_each(
        &mut journal,
        &mut summary,
        Phase::Run,
        operations,
        |op| match op.kind {
            OperationKind::CreateFolder => create_folder(recorded(op)?).map(|()| None),
            OperationKind::Trash => {
                if let Some(note) = blocked.get(op.id.as_str()) {
                    return Err((OutcomeKind::Skipped, note.to_string()));
                }
                ready_to_go(op, "since the Plan was made")
                    .map_err(|r| (OutcomeKind::Skipped, r))?;
                send_to_trash(trash, op).map(Some)
            }
            OperationKind::Move => {
                let destination = recorded(op)?;
                ready_to_go(op, "since the Plan was made")
                    .and_then(|()| folder_exists(op))
                    .map_err(|r| (OutcomeKind::Skipped, r))?;
                move_without_overwriting(&op.source, destination)
                    .map(|()| None)
                    .map_err(|r| (OutcomeKind::Failed, r))
            }
        },
    )?;
    journal.append(&Record::Finished)?;
    progress.finish(&plan.root);
    Ok(summary)
}

/// The longest a Run waits between progress reports.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Reports a Run's progress, at most every `PROGRESS_INTERVAL`.
struct Progress<F: FnMut(RunProgress)> {
    finished: usize,
    total: usize,
    last_report: Option<Instant>,
    report: F,
}

impl<F: FnMut(RunProgress)> Progress<F> {
    fn new(total: usize, report: F) -> Self {
        Progress {
            finished: 0,
            total,
            last_report: None,
            report,
        }
    }

    /// Called as the Run moves on to the next Operation, on `current`.
    fn starting(&mut self, current: &Path) {
        if self
            .last_report
            .is_none_or(|last| last.elapsed() >= PROGRESS_INTERVAL)
        {
            (self.report)(RunProgress {
                finished: self.finished,
                total: self.total,
                current: current.to_path_buf(),
            });
            self.last_report = Some(Instant::now());
        }
        self.finished += 1;
    }

    fn finish(mut self, root: &Path) {
        (self.report)(RunProgress {
            finished: self.total,
            total: self.total,
            current: root.to_path_buf(),
        });
    }
}

/// Reverses every done Operation of Session `session_id`, newest first,
/// using only its Journal in `journal_dir`. Never overwrites: an Operation
/// whose original location is taken is skipped, and can be retried by
/// undoing again.
pub fn undo(
    session_id: &str,
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    undo_selected(session_id, None, journal_dir, trash)
}

/// Like `undo`, for only the Operations in `operation_ids`. Operations that
/// aren't done (say, already undone) are left alone.
pub fn undo_operations(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    let only: HashSet<&str> = operation_ids.iter().map(String::as_str).collect();
    undo_selected(session_id, Some(&only), journal_dir, trash)
}

fn undo_selected(
    session_id: &str,
    only: Option<&HashSet<&str>>,
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let (session, mut journal) = reopen(journal_dir, session_id, trash)?;

    let newer = NewerSessions::read(journal_dir, &session.header.id)?;
    let mut summary = Summary::new(session.header.id.clone());
    // Operations this pass has settled: undone, or found unrecoverable.
    let mut settled: HashSet<String> = HashSet::new();
    apply_each(
        &mut journal,
        &mut summary,
        Phase::Undo,
        session.in_state(OperationState::Done, only).rev(),
        |op| {
            let result = undo_one(&session, &newer, &settled, op, trash);
            if !matches!(result, Err((OutcomeKind::Skipped, _))) {
                settled.insert(op.id.clone());
            }
            result.map(|()| None)
        },
    )?;
    Ok(summary)
}

/// Undoes one done Operation of `session`.
fn undo_one(
    session: &SessionJournal,
    newer: &NewerSessions,
    settled: &HashSet<String>,
    op: &JournaledOperation,
    trash: &dyn OsTrash,
) -> Result<(), NotDone> {
    let destination = recorded(op)?;
    if op.kind == OperationKind::CreateFolder {
        // Removed only once every item the Session moved into it is back,
        // or it would be made again, and left behind, when one comes back.
        let waiting = session.operations.iter().any(|other| {
            other.kind != OperationKind::CreateFolder
                && other.state == OperationState::Done
                && !settled.contains(&other.id)
                && other
                    .destination
                    .as_deref()
                    .is_some_and(|d| d.starts_with(destination))
        });
        if waiting {
            return Err((
                OutcomeKind::Skipped,
                "Items this Session moved into it aren't back yet, so it was left".into(),
            ));
        }
        return remove_created_folder(destination).map_err(|r| (OutcomeKind::Skipped, r));
    }
    newer.conflict(op).map_err(|r| (OutcomeKind::Skipped, r))?;
    let what = match op.kind {
        OperationKind::Trash if op.planned.is_folder => "trashed folder",
        OperationKind::Trash => "trashed copy",
        _ => "moved item",
    };
    // The OS reuses a name once the Trash is emptied, so what's there now may
    // be another item. One that can't be read yet (no Full Disk Access) is
    // left for `put_back` to report.
    let replaced = op.in_system_trash
        && !op.planned.is_folder
        && fs::symlink_metadata(destination).is_ok()
        && unchanged(destination, &op.planned, "").is_err();
    if !exists(destination) || replaced {
        let note = if op.in_system_trash {
            format!("It's no longer in the {TRASH_PLACE}")
        } else {
            format!("The {what} at {} is gone", destination.display())
        };
        return Err((OutcomeKind::Unrecoverable, note));
    }
    let left = |note: String| {
        (
            OutcomeKind::Skipped,
            format!("{note}; the {what} was left at {}", destination.display()),
        )
    };
    if op.planned.is_folder && op.source.is_dir() {
        // Undoing its files on their own made the folder again.
        merge_folder(destination, &op.source).map_err(left)?;
        if op.in_system_trash {
            trash.forget(destination);
        }
    } else if op.in_system_trash {
        trash.put_back(destination, &op.source).map_err(left)?;
    } else {
        move_without_overwriting(destination, &op.source).map_err(left)?;
    }
    Ok(())
}

/// What Sessions started after one Session did, as far as they still
/// stand (not undone): where they took items from, and where they put
/// them. Undoing the older Session must not undo around them.
struct NewerSessions {
    /// Lowercased paths a newer Session moved an item away from: that
    /// Session's ID and where the item went.
    moved_away: HashMap<String, (String, PathBuf)>,
    /// Lowercased paths a newer Session put an item at, and its ID.
    put_at: HashMap<String, String>,
}

impl NewerSessions {
    /// Reads the Journals of the Sessions started after `session_id`. One
    /// that can't be understood is passed over.
    fn read(journal_dir: &Path, session_id: &str) -> Result<NewerSessions, SessionError> {
        let mut newer = NewerSessions {
            moved_away: HashMap::new(),
            put_at: HashMap::new(),
        };
        let mut ids: Vec<String> = journal::session_ids(journal_dir)?
            .into_iter()
            .filter(|id| id.as_str() > session_id)
            .collect();
        // Oldest first, so the newest Session's word on a path wins.
        ids.sort();
        for id in ids {
            let session = match SessionJournal::read(journal_dir, &id) {
                Ok(session) => session,
                Err(SessionError::BadJournal { .. }) => continue,
                Err(e) => return Err(e),
            };
            for op in session.in_state(OperationState::Done, None) {
                let Some(destination) = &op.destination else {
                    continue;
                };
                if op.kind != OperationKind::CreateFolder {
                    newer
                        .moved_away
                        .insert(key(&op.source), (id.clone(), destination.clone()));
                }
                newer.put_at.insert(key(destination), id.clone());
            }
        }
        Ok(newer)
    }

    /// Fails with a note if a newer Session has since moved `op`'s item on,
    /// or put another item where `op` would put it back.
    fn conflict(&self, op: &JournaledOperation) -> Result<(), String> {
        let moved_on = op
            .destination
            .as_deref()
            .and_then(|destination| self.moved_away.get(&key(destination)));
        if let Some((id, to)) = moved_on {
            return Err(format!(
                "Session {id} has since moved it to {}; undo that Session first",
                to.display()
            ));
        }
        if let Some(id) = self.put_at.get(&key(&op.source)) {
            return Err(format!(
                "Session {id} has since put another item at {}; undo that Session first",
                op.source.display()
            ));
        }
        Ok(())
    }
}

/// `path` as a key that matches it in any letter case, erring towards
/// seeing a conflict.
fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// Carries out again the undone Operations of Session `session_id` that
/// are in `operation_ids`, in the order they first ran, sending each trashed
/// item to `trash` again, and putting each moved item back where the Session
/// put it. An item that changed since Undo put it back is skipped, as is one
/// whose destination is taken or that `trash` refuses; Operations that aren't
/// undone are left alone.
pub fn redo_operations(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let (session, mut journal) = reopen(journal_dir, session_id, trash)?;
    let only: HashSet<&str> = operation_ids.iter().map(String::as_str).collect();

    let mut summary = Summary::new(session.header.id.clone());
    apply_each(
        &mut journal,
        &mut summary,
        Phase::Redo,
        session.in_state(OperationState::Undone, Some(&only)),
        |op| match op.kind {
            OperationKind::CreateFolder => create_folder(recorded(op)?)
                .map(|()| None)
                .map_err(|(_, r)| (OutcomeKind::Skipped, r)),
            OperationKind::Trash => {
                ready_to_go(op, "since Undo put it back").map_err(|r| (OutcomeKind::Skipped, r))?;
                send_to_trash(trash, op).map(Some)
            }
            OperationKind::Move => {
                let destination = recorded(op)?;
                ready_to_go(op, "since Undo put it back")
                    .and_then(|()| folder_exists(op))
                    .and_then(|()| move_without_overwriting(&op.source, destination))
                    .map(|()| None)
                    .map_err(|r| (OutcomeKind::Skipped, r))
            }
        },
    )?;
    Ok(summary)
}

/// Session `session_id`'s Journal, read and open to add to, with every
/// pass a crash cut short settled first.
fn reopen(
    journal_dir: &Path,
    session_id: &str,
    trash: &dyn OsTrash,
) -> Result<(SessionJournal, Journal), SessionError> {
    let mut session = SessionJournal::read(journal_dir, session_id)?;
    let mut journal = Journal::reopen(journal_dir, &session.header.id)?;
    settle_interrupted(&mut session, &mut journal, trash)?;
    Ok((session, journal))
}

/// A crash between a change and its outcome record leaves an intent with
/// no outcome. If the disk is as that pass would leave it, the change
/// happened: journal it as done (dropping the OS's record after an Undo, as
/// it would have), so the Operation is in the state the disk is in. Otherwise
/// it never happened, and the Operation is as it was. A Trash whose landing
/// wasn't recorded is neither: see `may_be_in_trash`.
fn settle_interrupted(
    session: &mut SessionJournal,
    journal: &mut Journal,
    trash: &dyn OsTrash,
) -> io::Result<()> {
    let mut settled = Vec::new();
    for op in &session.operations {
        let Some(phase) = op.interrupted else {
            continue;
        };
        if op.may_be_in_trash() {
            continue;
        }
        let destination = op.destination.as_deref();
        let happened = match (op.kind, phase) {
            (OperationKind::CreateFolder, Phase::Run | Phase::Redo) => {
                destination.is_some_and(exists)
            }
            (OperationKind::CreateFolder, Phase::Undo) => !destination.is_some_and(exists),
            (OperationKind::Trash | OperationKind::Move, _) => op
                .route(phase)
                .is_some_and(|(from, to)| !exists(from) && exists(to)),
        };
        if !happened {
            continue;
        }
        if let (Phase::Undo, true, Some(landed)) = (phase, op.in_system_trash, destination) {
            trash.forget(landed);
        }
        settled.push(Record::Outcome {
            operation_id: op.id.clone(),
            phase,
            outcome: OutcomeKind::Done,
            reason: None,
            landed: None,
        });
    }
    for record in &settled {
        journal.append(record)?;
        session.apply(record);
    }
    Ok(())
}

/// Where `op`'s Journal says it goes. Only a Trash that hasn't landed goes
/// without, so one that needs it and has none is a damaged Journal.
fn recorded(op: &JournaledOperation) -> Result<&Path, NotDone> {
    op.destination.as_deref().ok_or_else(|| {
        (
            OutcomeKind::Skipped,
            "The Journal doesn't say where it went".to_string(),
        )
    })
}

/// Sends `op`'s item to `trash` and returns where it landed. A refusal leaves
/// it in place, so it is skipped; if it left all the same, the app can't say
/// where to, and doesn't guess.
fn send_to_trash(trash: &dyn OsTrash, op: &JournaledOperation) -> Result<PathBuf, NotDone> {
    trash.trash(&op.source).map_err(|note| {
        if exists(&op.source) {
            (OutcomeKind::Skipped, note)
        } else {
            (OutcomeKind::Unrecoverable, restore_by_hand_note())
        }
    })
}

/// For each of `operations`: journals `phase`'s intent, tries `attempt`,
/// journals how it ended, and adds that to `summary`.
fn apply_each<T: Borrow<JournaledOperation>>(
    journal: &mut Journal,
    summary: &mut Summary,
    phase: Phase,
    operations: impl Iterator<Item = T>,
    mut attempt: impl FnMut(&JournaledOperation) -> Result<Option<PathBuf>, NotDone>,
) -> io::Result<()> {
    for op in operations {
        let op = op.borrow();
        journal.append(&op.intent(phase))?;
        let (outcome, note, landed) = match attempt(op) {
            Ok(landed) => (OutcomeKind::Done, None, landed),
            Err((outcome, note)) => (outcome, Some(note), None),
        };
        journal.append(&Record::Outcome {
            operation_id: op.id.clone(),
            phase,
            outcome,
            reason: note.clone(),
            landed: landed.clone(),
        })?;
        summary.push(outcome, op.outcome(landed, note));
    }
    Ok(())
}

pub(crate) fn exists(path: &Path) -> bool {
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
    let size = if is_package {
        total_size(path)
    } else {
        metadata.len()
    };
    if size != expected.size {
        return Err(format!("Its size changed {since}"));
    }
    match metadata.modified() {
        Ok(modified) if millis(modified) == expected.modified => Ok(()),
        _ => Err(format!("It was modified {since}")),
    }
}

/// Fails with a note if `op`'s item isn't as planned: for an emptied folder,
/// if it holds anything but OS Metadata Files and empty folders; for
/// anything else, if it changed. `since` ends the note.
fn ready_to_go(op: &JournaledOperation, since: &str) -> Result<(), String> {
    if !op.planned.is_folder {
        return unchanged(&op.source, &op.planned, since);
    }
    match fs::symlink_metadata(&op.source) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Err(format!("It was replaced by something else {since}")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err("It no longer exists".into()),
        Err(e) => return Err(format!("It couldn't be read: {e}")),
    }
    match first_kept(&op.source) {
        Ok(None) => Ok(()),
        Ok(Some(item)) => {
            let name = item.strip_prefix(&op.source).unwrap_or(&item);
            Err(format!("It isn't empty: {} is in it", name.display()))
        }
        Err(e) => Err(format!("It couldn't be read: {e}")),
    }
}

/// The first item in `folder` that stops it counting as empty: anything
/// but an OS Metadata File or a folder that is empty too.
fn first_kept(folder: &Path) -> io::Result<Option<PathBuf>> {
    // A folder in the Scan Root the user chose; no path comes from a web request.
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    for child in fs::read_dir(folder)? {
        let child = child?;
        let kind = child.file_type()?;
        let path = child.path();
        if kind.is_file() && is_os_metadata_file(&child.file_name().to_string_lossy()) {
            continue;
        }
        if !kind.is_dir() {
            return Ok(Some(path));
        }
        if let Some(kept) = first_kept(&path)? {
            return Ok(Some(kept));
        }
    }
    Ok(None)
}

/// `destination`, or if its name is taken (in any letter case), the first
/// free name with a Clash Suffix in the same folder.
fn free_destination(destination: &Path) -> PathBuf {
    let (Some(folder), Some(name)) = (destination.parent(), destination.file_name()) else {
        return destination.to_path_buf();
    };
    folder.join(clash::free_name(
        &name.to_string_lossy(),
        &clash::names_in(folder),
    ))
}

/// Fails with a note if a Move's destination folder is missing: Create
/// Folder makes it, and only if that Operation was approved.
fn folder_exists(op: &JournaledOperation) -> Result<(), String> {
    match op.destination.as_deref().and_then(Path::parent) {
        Some(folder) if !folder.is_dir() => {
            Err(format!("The folder {} doesn't exist", folder.display()))
        }
        _ => Ok(()),
    }
}

/// Creates the folder `path`, whose parent must exist.
fn create_folder(path: &Path) -> Result<(), NotDone> {
    if exists(path) {
        return Err((
            OutcomeKind::Skipped,
            format!("{} already exists", path.display()),
        ));
    }
    fs::create_dir(path).map_err(|e| {
        (
            OutcomeKind::Failed,
            format!("Couldn't create {}: {e}", path.display()),
        )
    })
}

/// Removes the folder `path` that a Session created, only if it is empty,
/// so nothing added to it since can be lost. One already gone is fine.
fn remove_created_folder(path: &Path) -> Result<(), String> {
    if !exists(path) {
        return Ok(());
    }
    fs::remove_dir(path).map_err(|e| {
        if fs::read_dir(path).is_ok_and(|mut children| children.next().is_some()) {
            "It isn't empty, so it was left".to_string()
        } else {
            format!("Couldn't remove it: {e}")
        }
    })
}

/// Moves `from` to `to` with a same-disk rename, creating `to`'s folders.
/// Never replaces anything already at `to`, and never copies.
pub(crate) fn move_without_overwriting(from: &Path, to: &Path) -> Result<(), String> {
    if fs::symlink_metadata(to).is_ok() {
        return Err(format!("{} already exists", to.display()));
    }
    let moved = to
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::rename(from, to));
    moved.map_err(|e| match e.kind() {
        io::ErrorKind::CrossesDevices => format!(
            "{} is on a different disk from {}",
            from.display(),
            to.display()
        ),
        _ => format!("Couldn't move it to {}: {e}", to.display()),
    })
}

/// Moves everything in the folder `from` into the folder `to`, then
/// removes `from` once it is empty. Never replaces anything in `to`: an
/// item whose name is taken there stays in `from`, which is then kept.
fn merge_folder(from: &Path, to: &Path) -> Result<(), String> {
    let children = fs::read_dir(from).map_err(|e| format!("Couldn't read it: {e}"))?;
    let mut kept = None;
    for child in children {
        let child = child.map_err(|e| format!("Couldn't read it: {e}"))?;
        let target = to.join(child.file_name());
        let both_folders = child.file_type().is_ok_and(|t| t.is_dir()) && target.is_dir();
        let merged = if both_folders {
            merge_folder(&child.path(), &target)
        } else {
            move_without_overwriting(&child.path(), &target)
        };
        if let Err(note) = merged {
            kept.get_or_insert(note);
        }
    }
    match kept {
        Some(note) => Err(note),
        None => fs::remove_dir(from).map_err(|e| format!("Couldn't remove it: {e}")),
    }
}
