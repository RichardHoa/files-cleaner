//! Past Sessions as their Journals tell them, plus Undo and Redo of a whole
//! Session or of chosen Operations.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::TrashDestination;
use crate::executor::{move_without_overwriting, unchanged, Outcome, SourceState};
use crate::journal::{self, lock_sessions, Journal, Record, SessionError};
use crate::planner::OperationKind;
use crate::protection::HOLDING_FOLDER_NAME;

/// One past Session, for the Session list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub scan_root: PathBuf,
    pub trash_destination: TrashDestination,
    /// Milliseconds since the Unix epoch.
    pub started_at: i64,
    pub status: SessionStatus,
    /// Operations that were carried out, whether or not undone since.
    pub done: usize,
    /// Of those, how many are undone now.
    pub restored: usize,
    /// Of those, how many Undo could still reverse: not undone, and not
    /// found gone by an earlier Undo.
    pub left_to_undo: usize,
    /// Bytes of the Operations carried out; unknown sizes count as nothing.
    pub total_size: u64,
    /// How many Operations of each kind were carried out.
    pub kind_counts: BTreeMap<OperationKind, usize>,
}

/// One page of the Session list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPage {
    /// Every Journal, on any page; one that turns out unreadable is still
    /// counted, as only the requested page's Journals are read.
    pub total: usize,
    /// Newest first.
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    /// Nothing is undone.
    Completed,
    /// It stopped before its last Operation was recorded.
    Crashed,
    /// Every Operation carried out is undone.
    Undone,
    /// Some Operations are undone, but not all.
    PartlyUndone,
}

/// A Session and each of its Operations, for the Session screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetails {
    pub info: SessionInfo,
    /// In the order they ran.
    pub operations: Vec<SessionOperation>,
}

/// One Operation of a past Session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOperation {
    pub id: String,
    pub kind: OperationKind,
    /// Where the item was before the Operation.
    pub source: PathBuf,
    /// Where the Operation put it.
    pub destination: PathBuf,
    /// Unknown in Journals written before Redo existed.
    pub size: Option<u64>,
    pub state: OperationState,
    /// Why it was skipped or failed, or why the last Undo or Redo of it
    /// didn't happen.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    /// Journaled as about to happen, with no outcome after: the Session
    /// stopped there.
    Started,
    /// Carried out, and not undone (or redone since).
    Done,
    Skipped,
    Failed,
    /// Carried out, then undone. Redo can carry it out again.
    Undone,
    /// Carried out, but Undo found the trashed copy gone.
    Unrecoverable,
}

/// What happened when Operations were undone.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoSummary {
    pub session_id: String,
    pub restored: Vec<Outcome>,
    /// Not reversed, but could be later: something now sits at the
    /// original location, or the move failed.
    pub skipped: Vec<Outcome>,
    /// Not reversed, and never can be: the trashed copy is gone.
    pub unrecoverable: Vec<Outcome>,
}

/// What happened when undone Operations were carried out again.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedoSummary {
    pub session_id: String,
    pub redone: Vec<Outcome>,
    /// Not carried out: the item changed since Undo put it back, or its
    /// destination is taken.
    pub skipped: Vec<Outcome>,
}

/// One Operation as its Journal records tell it.
struct JournaledOperation {
    id: String,
    kind: OperationKind,
    source: PathBuf,
    destination: PathBuf,
    planned: Option<SourceState>,
    state: OperationState,
    reason: Option<String>,
}

impl JournaledOperation {
    fn outcome(&self, reason: Option<String>) -> Outcome {
        Outcome {
            operation_id: self.id.clone(),
            kind: self.kind,
            source: self.source.clone(),
            destination: self.destination.clone(),
            reason,
        }
    }
}

/// A Session's Journal, read.
struct SessionJournal {
    id: String,
    scan_root: PathBuf,
    trash_destination: TrashDestination,
    started_at: i64,
    finished: bool,
    /// In the order they ran.
    operations: Vec<JournaledOperation>,
}

impl SessionJournal {
    fn read(journal_dir: &Path, id: &str) -> Result<SessionJournal, SessionError> {
        let records = journal::read(journal_dir, id)?;
        let Some(Record::Session { id, scan_root, trash_destination, started_at }) = records.first().cloned() else {
            unreachable!("journal::read checks the first record");
        };
        let mut session =
            SessionJournal { id, scan_root, trash_destination, started_at, finished: false, operations: Vec::new() };
        for record in records {
            // An outcome with no intent before it is ignored.
            let (operation_id, state, reason) = match record {
                Record::Intent { operation_id, kind, source, destination, planned } => {
                    session.operations.push(JournaledOperation {
                        id: operation_id,
                        kind,
                        source,
                        destination,
                        planned,
                        state: OperationState::Started,
                        reason: None,
                    });
                    continue;
                }
                Record::Finished => {
                    session.finished = true;
                    continue;
                }
                Record::Done { operation_id } | Record::Redone { operation_id } => {
                    (operation_id, Some(OperationState::Done), None)
                }
                Record::Skipped { operation_id, reason } => (operation_id, Some(OperationState::Skipped), Some(reason)),
                Record::Failed { operation_id, reason } => (operation_id, Some(OperationState::Failed), Some(reason)),
                Record::Restored { operation_id } => (operation_id, Some(OperationState::Undone), None),
                Record::Unrecoverable { operation_id, reason } => {
                    (operation_id, Some(OperationState::Unrecoverable), Some(reason))
                }
                // The state stays as it was; the reason says why.
                Record::UndoSkipped { operation_id, reason } | Record::RedoSkipped { operation_id, reason } => {
                    (operation_id, None, Some(reason))
                }
                Record::Session { .. }
                | Record::UndoStarted
                | Record::UndoIntent { .. }
                | Record::UndoFinished
                | Record::RedoIntent { .. } => continue,
            };
            if let Some(op) = session.operations.iter_mut().rev().find(|op| op.id == operation_id) {
                op.state = state.unwrap_or(op.state);
                op.reason = reason;
            }
        }
        Ok(session)
    }

    fn info(&self) -> SessionInfo {
        let carried_out = self.operations.iter().filter(|op| {
            matches!(op.state, OperationState::Done | OperationState::Undone | OperationState::Unrecoverable)
        });
        let mut done = 0;
        let mut total_size = 0;
        let mut kind_counts = BTreeMap::new();
        for op in carried_out {
            done += 1;
            total_size += op.planned.map_or(0, |p| p.size);
            *kind_counts.entry(op.kind).or_insert(0) += 1;
        }
        let in_state = |state| self.operations.iter().filter(|op| op.state == state).count();
        let restored = in_state(OperationState::Undone);
        let left_to_undo = in_state(OperationState::Done);
        let status = if restored > 0 && restored == done {
            SessionStatus::Undone
        } else if restored > 0 {
            SessionStatus::PartlyUndone
        } else if self.finished {
            SessionStatus::Completed
        } else {
            SessionStatus::Crashed
        };
        SessionInfo {
            id: self.id.clone(),
            scan_root: self.scan_root.clone(),
            trash_destination: self.trash_destination,
            started_at: self.started_at,
            status,
            done,
            restored,
            left_to_undo,
            total_size,
            kind_counts,
        }
    }

    /// The Operations in `state`, limited to `only` if given.
    fn in_state<'a>(
        &'a self,
        state: OperationState,
        only: Option<&'a HashSet<&str>>,
    ) -> impl DoubleEndedIterator<Item = &'a JournaledOperation> {
        self.operations
            .iter()
            .filter(move |op| op.state == state && only.is_none_or(|ids| ids.contains(op.id.as_str())))
    }
}

/// The Sessions journaled in `journal_dir`, newest first, skipping
/// `offset` and returning at most `limit`, plus how many there are in all.
/// Session IDs sort by start time, so only the Journals on the page are
/// read. A Journal that can't be understood (say, left empty by a crash) is
/// left out, so it doesn't hide the others; its page is one short.
pub fn list_sessions(journal_dir: &Path, offset: usize, limit: usize) -> Result<SessionPage, SessionError> {
    let mut ids = journal::session_ids(journal_dir)?;
    ids.sort_by(|a, b| start_order(b).cmp(&start_order(a)));
    let mut sessions = Vec::new();
    for id in ids.iter().skip(offset).take(limit) {
        match SessionJournal::read(journal_dir, id) {
            Ok(session) => sessions.push(session.info()),
            Err(SessionError::BadJournal { .. }) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(SessionPage { total: ids.len(), sessions })
}

/// Sorts Session IDs by when they started: `<time>Z`, then `<time>Z-2`,
/// `<time>Z-3`, … for Sessions started in the same millisecond.
fn start_order(id: &str) -> (&str, u64) {
    if let Some((time, n)) = id.rsplit_once("Z-") {
        if let Ok(n) = n.parse() {
            return (time, n);
        }
    }
    (id.strip_suffix('Z').unwrap_or(id), 1)
}

/// Session `session_id` and each of its Operations, from its Journal.
pub fn session_details(session_id: &str, journal_dir: &Path) -> Result<SessionDetails, SessionError> {
    let session = SessionJournal::read(journal_dir, session_id)?;
    let operations = session
        .operations
        .iter()
        .map(|op| SessionOperation {
            id: op.id.clone(),
            kind: op.kind,
            source: op.source.clone(),
            destination: op.destination.clone(),
            size: op.planned.map(|p| p.size),
            state: op.state,
            reason: op.reason.clone(),
        })
        .collect();
    Ok(SessionDetails { info: session.info(), operations })
}

/// Reverses every done Operation of Session `session_id`, newest first,
/// using only its Journal in `journal_dir`. Never overwrites: an Operation
/// whose original location is taken is skipped, and can be retried by
/// undoing again.
pub fn undo(session_id: &str, journal_dir: &Path) -> Result<UndoSummary, SessionError> {
    undo_selected(session_id, None, journal_dir)
}

/// Like `undo`, for only the Operations in `operation_ids`. Operations that
/// aren't done (say, already undone) are left alone.
pub fn undo_operations(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
) -> Result<UndoSummary, SessionError> {
    let only: HashSet<&str> = operation_ids.iter().map(String::as_str).collect();
    undo_selected(session_id, Some(&only), journal_dir)
}

fn undo_selected(
    session_id: &str,
    only: Option<&HashSet<&str>>,
    journal_dir: &Path,
) -> Result<UndoSummary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let session = SessionJournal::read(journal_dir, session_id)?;
    let mut journal = Journal::reopen(journal_dir, &session.id)?;
    journal.append(&Record::UndoStarted)?;

    let holding = session.scan_root.join(HOLDING_FOLDER_NAME);
    let mut summary = UndoSummary {
        session_id: session.id.clone(),
        restored: Vec::new(),
        skipped: Vec::new(),
        unrecoverable: Vec::new(),
    };
    for op in session.in_state(OperationState::Done, only).rev() {
        let operation_id = op.id.clone();
        journal.append(&Record::UndoIntent { operation_id: operation_id.clone() })?;
        let gone = matches!(fs::symlink_metadata(&op.destination), Err(e) if e.kind() == io::ErrorKind::NotFound);
        if gone {
            let reason = format!("The trashed copy at {} is gone", op.destination.display());
            journal.append(&Record::Unrecoverable { operation_id, reason: reason.clone() })?;
            summary.unrecoverable.push(op.outcome(Some(reason)));
        } else if let Err(reason) = move_without_overwriting(&op.destination, &op.source) {
            let reason = format!("{reason}; the trashed copy was left at {}", op.destination.display());
            journal.append(&Record::UndoSkipped { operation_id, reason: reason.clone() })?;
            summary.skipped.push(op.outcome(Some(reason)));
        } else {
            journal.append(&Record::Restored { operation_id })?;
            remove_empty_folders(op.destination.parent(), &holding);
            summary.restored.push(op.outcome(None));
        }
    }
    journal.append(&Record::UndoFinished)?;
    Ok(summary)
}

/// Carries out again the undone Operations of Session `session_id` that
/// are in `operation_ids`, in the order they first ran, putting each item
/// back where the Session put it. An item that changed since Undo put it
/// back is skipped, as is one whose destination is taken; Operations that
/// aren't undone are left alone.
pub fn redo_operations(
    session_id: &str,
    operation_ids: &[String],
    journal_dir: &Path,
) -> Result<RedoSummary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let session = SessionJournal::read(journal_dir, session_id)?;
    let mut journal = Journal::reopen(journal_dir, &session.id)?;
    let only: HashSet<&str> = operation_ids.iter().map(String::as_str).collect();

    let mut summary = RedoSummary { session_id: session.id.clone(), redone: Vec::new(), skipped: Vec::new() };
    for op in session.in_state(OperationState::Undone, Some(&only)) {
        let operation_id = op.id.clone();
        journal.append(&Record::RedoIntent { operation_id: operation_id.clone() })?;
        let redone = match &op.planned {
            None => Err("This Session was recorded before Redo existed, so the app can't check the item".into()),
            Some(planned) => unchanged(&op.source, planned, "since Undo put it back")
                .and_then(|()| move_without_overwriting(&op.source, &op.destination)),
        };
        match redone {
            Ok(()) => {
                journal.append(&Record::Redone { operation_id })?;
                summary.redone.push(op.outcome(None));
            }
            Err(reason) => {
                journal.append(&Record::RedoSkipped { operation_id, reason: reason.clone() })?;
                summary.skipped.push(op.outcome(Some(reason)));
            }
        }
    }
    Ok(summary)
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
