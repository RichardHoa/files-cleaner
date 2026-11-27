//! Every step is journaled before it happens, so a crash can never hide a
//! change.

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::clash;
use crate::journal::{lock_sessions, Journal, Journals, OutcomeKind, Phase, Record, SessionError};
use crate::os_trash::{OsTrash, TRASH_PLACE};
use crate::planner::{Operation, OperationKind, Plan, SourceState};
use crate::protection::is_os_metadata_file;
use crate::scanner::{exists, millis, total_size};
use crate::session_journal::{
    restore_by_hand_note, JournaledOperation, OperationState, SessionJournal,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// `None` after a Run that did nothing, which keeps no Session.
    pub session_id: Option<String>,
    pub done: Vec<Outcome>,
    /// Can be tried again later, unlike `failed` and `unrecoverable`.
    pub skipped: Vec<Outcome>,
    pub failed: Vec<Outcome>,
    /// The trashed copy is gone, or where it landed was never recorded.
    pub unrecoverable: Vec<Outcome>,
}

impl Summary {
    fn new(session_id: String) -> Self {
        Summary {
            session_id: Some(session_id),
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub operation_id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    /// `None` for a Trash that hasn't landed: only the OS decides where.
    pub destination: Option<PathBuf>,
    pub note: Option<String>,
    pub note_kind: Option<NoteKind>,
}

fn outcome_of(op: &JournaledOperation, landed: Option<PathBuf>, note: Option<String>) -> Outcome {
    Outcome {
        operation_id: op.id.clone(),
        kind: op.kind,
        source: op.source.clone(),
        destination: landed.or_else(|| op.destination.clone()),
        note,
        note_kind: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteKind {
    TrashRefused,
    GoneFromTrash,
    NameTaken,
    Changed,
}

struct NotDone {
    outcome: OutcomeKind,
    note: String,
    kind: Option<NoteKind>,
}

impl NotDone {
    fn new(outcome: OutcomeKind, note: impl Into<String>) -> Self {
        NotDone {
            outcome,
            note: note.into(),
            kind: None,
        }
    }

    fn skipped(note: impl Into<String>) -> Self {
        NotDone::new(OutcomeKind::Skipped, note)
    }

    fn failed(note: impl Into<String>) -> Self {
        NotDone::new(OutcomeKind::Failed, note)
    }

    fn unrecoverable(note: impl Into<String>) -> Self {
        NotDone::new(OutcomeKind::Unrecoverable, note)
    }

    fn because(self, kind: NoteKind) -> Self {
        NotDone {
            kind: Some(kind),
            ..self
        }
    }

    fn because_if(self, applies: bool, kind: NoteKind) -> Self {
        if applies {
            self.because(kind)
        } else {
            self
        }
    }
}

/// An approved Operation the Plan marks as an error is journaled as skipped, so
/// the user still sees it listed. A Run that did nothing keeps no Session, as
/// there is nothing to undo.
pub fn run(
    plan: &Plan,
    approved: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    run_with_progress(plan, approved, journal_dir, trash, |_| {})
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProgress {
    pub finished: usize,
    pub total: usize,
    pub current: PathBuf,
}

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
        approved.contains(op.id.as_str())
            && (op.error.is_none() || op.kind != OperationKind::CreateFolder)
    };
    let total = plan.operations.iter().filter(runs).count();
    let (header, mut journal) = Journal::create(journal_dir, plan.root.clone())?;
    let blocked: HashMap<&str, &str> = plan
        .operations
        .iter()
        .filter_map(|op| Some((op.id.as_str(), op.error.as_deref()?)))
        .collect();

    // Lazy, so each destination is checked once the Operations before it have
    // run.
    let operations = plan.operations.iter().filter(runs).map(|op| {
        to_run(
            &op.id,
            op.kind,
            &op.source,
            op.destination_path.as_deref(),
            op.planned,
        )
    });

    let mut summary = Summary::new(header.id.clone());
    let mut progress = Progress::new(total, on_progress);
    apply_each(
        &mut journal,
        &mut summary,
        &mut progress,
        Phase::Run,
        operations,
        |op| {
            if let Some(note) = blocked.get(op.id.as_str()) {
                // A Trash is only blocked by the System Trash.
                let skipped = NotDone::skipped(*note);
                return Err(match op.kind {
                    OperationKind::Trash => skipped.because(NoteKind::TrashRefused),
                    _ => skipped,
                });
            }
            run_one(op, trash)
        },
    )?;
    journal.append(&Record::Finished)?;
    if summary.done.is_empty() {
        journal.discard()?;
        summary.session_id = None;
    }
    progress.finish(&plan.root);
    Ok(summary)
}

/// Worked out just before journaling, since the destination may have been taken
/// since planning.
fn to_run(
    id: &str,
    kind: OperationKind,
    source: &Path,
    destination: Option<&Path>,
    planned: SourceState,
) -> JournaledOperation {
    JournaledOperation {
        id: id.to_string(),
        kind,
        source: source.to_path_buf(),
        destination: match kind {
            OperationKind::Move => destination.map(free_destination),
            OperationKind::Trash | OperationKind::CreateFolder => {
                destination.map(Path::to_path_buf)
            }
        },
        in_system_trash: false,
        planned,
        state: OperationState::Started,
        note: None,
        interrupted: None,
    }
}

fn run_one(op: &JournaledOperation, trash: &dyn OsTrash) -> Result<Option<PathBuf>, NotDone> {
    match op.kind {
        OperationKind::CreateFolder => create_folder(recorded(op)?).map(|()| None),
        OperationKind::Trash => {
            still_as_planned(op, "since the Plan was made")?;
            send_to_trash(trash, op).map(Some)
        }
        OperationKind::Move => {
            let destination = recorded(op)?;
            still_as_planned(op, "since the Plan was made")?;
            folder_exists(op).map_err(NotDone::skipped)?;
            move_without_overwriting(&op.source, destination)
                .map(|()| None)
                .map_err(NotDone::failed)
        }
    }
}

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

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

    fn finish(&mut self, root: &Path) {
        (self.report)(RunProgress {
            finished: self.total,
            total: self.total,
            current: root.to_path_buf(),
        });
    }
}

/// Newest first, as later Operations may depend on earlier ones. Never
/// overwrites; skipped ones can be retried by undoing again.
pub fn undo(
    session_id: &str,
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    undo_with_progress(session_id, None, journal_dir, trash, |_| {})
}

pub fn undo_operations(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    undo_with_progress(session_id, Some(operation_ids), journal_dir, trash, |_| {})
}

/// All of the Session's done Operations when `operation_ids` is `None`.
pub fn undo_with_progress(
    session_id: &str,
    operation_ids: Option<&[String]>,
    journal_dir: &Path,
    trash: &dyn OsTrash,
    on_progress: impl FnMut(RunProgress),
) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let (session, mut journal) = reopen(journal_dir, session_id, trash)?;
    let only: Option<HashSet<&str>> =
        operation_ids.map(|ids| ids.iter().map(String::as_str).collect());

    let newer = NewerSessions::read(journal.journals(), &session.header.id)?;
    let operations: Vec<&JournaledOperation> = session
        .in_state(OperationState::Done, only.as_ref())
        .rev()
        .collect();
    let mut summary = Summary::new(session.header.id.clone());
    let mut progress = Progress::new(operations.len(), on_progress);
    let mut settled: HashSet<String> = HashSet::new();
    apply_each(
        &mut journal,
        &mut summary,
        &mut progress,
        Phase::Undo,
        operations.into_iter(),
        |op| {
            let result = undo_one(&session, &newer, &settled, op, trash);
            if !matches!(&result, Err(not_done) if not_done.outcome == OutcomeKind::Skipped) {
                settled.insert(op.id.clone());
            }
            result.map(|()| None)
        },
    )?;
    progress.finish(&session.header.scan_root);
    Ok(summary)
}

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
            return Err(NotDone::skipped(
                "Items this Session moved into it aren't back yet, so it was left",
            ));
        }
        return remove_created_folder(destination).map_err(NotDone::skipped);
    }
    newer.conflict(op).map_err(NotDone::skipped)?;
    let what = match op.kind {
        OperationKind::Trash if op.planned.is_folder => "trashed folder",
        OperationKind::Trash => "trashed copy",
        _ => "moved item",
    };
    // The OS reuses a name once the Trash is emptied, so this may be another
    // item. One that can't be read yet (no Full Disk Access) is left for
    // `put_back` to report.
    let replaced = op.in_system_trash
        && !op.planned.is_folder
        && fs::symlink_metadata(destination).is_ok()
        && unchanged(destination, &op.planned, "").is_err();
    if !exists(destination) || replaced {
        return Err(if op.in_system_trash {
            NotDone::unrecoverable(format!("It's no longer in the {TRASH_PLACE}"))
                .because(NoteKind::GoneFromTrash)
        } else {
            NotDone::unrecoverable(format!("The {what} at {} is gone", destination.display()))
        });
    }
    // Nothing is ever replaced, so an item where it goes back is what stopped
    // it.
    let left = |note: String| {
        let skipped = NotDone::skipped(format!(
            "{note}; the {what} was left at {}",
            destination.display()
        ));
        if exists(&op.source) {
            skipped.because(NoteKind::NameTaken)
        } else {
            skipped
        }
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

/// Undoing an older Session must not undo around what newer ones did since.
struct NewerSessions {
    moved_away: HashMap<String, (String, PathBuf)>,
    put_at: HashMap<String, String>,
}

impl NewerSessions {
    /// An unreadable Journal is passed over, so it can't block every Undo.
    fn read(journals: &Journals, session_id: &str) -> Result<NewerSessions, SessionError> {
        let mut newer = NewerSessions {
            moved_away: HashMap::new(),
            put_at: HashMap::new(),
        };
        let mut ids: Vec<String> = journals
            .session_ids()?
            .into_iter()
            .filter(|id| id.as_str() > session_id)
            .collect();
        // Oldest first, so the newest Session's word on a path wins.
        ids.sort();
        for id in ids {
            let session = match SessionJournal::read(journals, &id) {
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

/// Letter case is ignored, erring towards seeing a conflict.
fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

pub fn redo_operations(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    redo_with_progress(session_id, operation_ids, journal_dir, trash, |_| {})
}

/// In the order they first ran, as later Operations may depend on earlier ones.
pub fn redo_with_progress(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
    on_progress: impl FnMut(RunProgress),
) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let (session, mut journal) = reopen(journal_dir, session_id, trash)?;
    let only: HashSet<&str> = operation_ids.iter().map(String::as_str).collect();

    let operations: Vec<&JournaledOperation> = session
        .in_state(OperationState::Undone, Some(&only))
        .collect();
    let mut summary = Summary::new(session.header.id.clone());
    let mut progress = Progress::new(operations.len(), on_progress);
    apply_each(
        &mut journal,
        &mut summary,
        &mut progress,
        Phase::Redo,
        operations.into_iter(),
        |op| match op.kind {
            OperationKind::CreateFolder => create_folder(recorded(op)?)
                .map(|()| None)
                .map_err(|not_done| NotDone::skipped(not_done.note)),
            OperationKind::Trash => {
                still_as_planned(op, "since Undo put it back")?;
                send_to_trash(trash, op).map(Some)
            }
            OperationKind::Move => {
                let destination = recorded(op)?;
                still_as_planned(op, "since Undo put it back")?;
                folder_exists(op).map_err(NotDone::skipped)?;
                move_without_overwriting(&op.source, destination)
                    .map(|()| None)
                    .map_err(|note| {
                        NotDone::skipped(note).because_if(exists(destination), NoteKind::NameTaken)
                    })
            }
        },
    )?;
    progress.finish(&session.header.scan_root);
    Ok(summary)
}

pub fn retry_operations(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
) -> Result<Summary, SessionError> {
    retry_with_progress(session_id, operation_ids, journal_dir, trash, |_| {})
}

/// Journaled as another Run pass of the same Session, so one Undo still
/// reverses it all.
pub fn retry_with_progress(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
    trash: &dyn OsTrash,
    on_progress: impl FnMut(RunProgress),
) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let (session, mut journal) = reopen(journal_dir, session_id, trash)?;
    let only: HashSet<&str> = operation_ids.iter().map(String::as_str).collect();

    let retried = |op: &&JournaledOperation| {
        matches!(op.state, OperationState::Skipped | OperationState::Failed)
            && only.contains(op.id.as_str())
    };
    let total = session.operations.iter().filter(retried).count();
    let operations = session.operations.iter().filter(retried).map(|op| {
        to_run(
            &op.id,
            op.kind,
            &op.source,
            op.destination.as_deref(),
            op.planned,
        )
    });
    let mut summary = Summary::new(session.header.id.clone());
    let mut progress = Progress::new(total, on_progress);
    apply_each(
        &mut journal,
        &mut summary,
        &mut progress,
        Phase::Run,
        operations,
        |op| run_one(op, trash),
    )?;
    progress.finish(&session.header.scan_root);
    Ok(summary)
}

fn reopen(
    journal_dir: &Path,
    session_id: &str,
    trash: &dyn OsTrash,
) -> Result<(SessionJournal, Journal), SessionError> {
    let journals = Journals::open(journal_dir)?;
    let mut session = SessionJournal::read(&journals, session_id)?;
    let mut journal = journals.reopen(&session.header.id)?;
    settle_interrupted(&mut session, &mut journal, trash)?;
    Ok((session, journal))
}

/// A crash leaves an intent with no outcome, so the disk decides whether the
/// change happened. An unlanded Trash can't be told: see `may_be_in_trash`.
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

/// Only an unlanded Trash goes without, so any other missing destination is a
/// damaged Journal.
fn recorded(op: &JournaledOperation) -> Result<&Path, NotDone> {
    op.destination
        .as_deref()
        .ok_or_else(|| NotDone::skipped("The Journal doesn't say where it went"))
}

/// If the item left despite a refusal, the app doesn't guess where to.
fn send_to_trash(trash: &dyn OsTrash, op: &JournaledOperation) -> Result<PathBuf, NotDone> {
    trash.trash(&op.source).map_err(|note| {
        if exists(&op.source) {
            NotDone::skipped(note).because(NoteKind::TrashRefused)
        } else {
            NotDone::unrecoverable(restore_by_hand_note())
        }
    })
}

fn apply_each<T: Borrow<JournaledOperation>>(
    journal: &mut Journal,
    summary: &mut Summary,
    progress: &mut Progress<impl FnMut(RunProgress)>,
    phase: Phase,
    operations: impl Iterator<Item = T>,
    mut attempt: impl FnMut(&JournaledOperation) -> Result<Option<PathBuf>, NotDone>,
) -> io::Result<()> {
    for op in operations {
        let op = op.borrow();
        progress.starting(&op.source);

        journal.append(&op.intent(phase))?;

        let (outcome, note, kind, landed) = match attempt(op) {
            Ok(landed) => (OutcomeKind::Done, None, None, landed),
            Err(not_done) => (not_done.outcome, Some(not_done.note), not_done.kind, None),
        };

        journal.append(&Record::Outcome {
            operation_id: op.id.clone(),
            phase,
            outcome,
            reason: note.clone(),
            landed: landed.clone(),
        })?;

        summary.push(
            outcome,
            Outcome {
                note_kind: kind,
                ..outcome_of(op, landed, note)
            },
        );
    }
    Ok(())
}

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

fn still_as_planned(op: &JournaledOperation, since: &str) -> Result<(), NotDone> {
    ready_to_go(op, since)
        .map_err(|note| NotDone::skipped(note).because_if(exists(&op.source), NoteKind::Changed))
}

/// An emptied folder's own date changes as its items leave, so it is checked by
/// what it still holds.
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

/// Letter case is ignored, as for every Clash Suffix.
fn free_destination(destination: &Path) -> PathBuf {
    let (Some(folder), Some(name)) = (destination.parent(), destination.file_name()) else {
        return destination.to_path_buf();
    };
    folder.join(clash::free_name(
        &name.to_string_lossy(),
        &clash::names_in(folder),
    ))
}

/// Never made here: the user may not have approved its Create Folder.
fn folder_exists(op: &JournaledOperation) -> Result<(), String> {
    match op.destination.as_deref().and_then(Path::parent) {
        Some(folder) if !folder.is_dir() => {
            Err(format!("The folder {} doesn't exist", folder.display()))
        }
        _ => Ok(()),
    }
}

fn create_folder(path: &Path) -> Result<(), NotDone> {
    if exists(path) {
        return Err(NotDone::skipped(format!(
            "{} already exists",
            path.display()
        )));
    }
    fs::create_dir(path)
        .map_err(|e| NotDone::failed(format!("Couldn't create {}: {e}", path.display())))
}

/// Only if empty, so nothing added to it since can be lost.
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

/// A rename only: a copy across disks could be cut short halfway.
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

/// Items whose names are taken stay in `from`, as nothing is ever replaced.
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
