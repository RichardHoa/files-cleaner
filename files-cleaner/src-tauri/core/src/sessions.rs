//! Past Sessions as their Journals tell them.

use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::config::TrashDestination;
use crate::executor::Outcome;
use crate::journal::{self, OutcomeKind, Phase, Record, SessionError, SessionHeader};
use crate::planner::{OperationKind, SourceState};

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
    /// Bytes of the Operations carried out.
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
    /// The names from the Scan Root down to the source, its own name last.
    pub relative_path: Vec<String>,
    /// The names from the Scan Root down to the destination, its own name
    /// last.
    pub relative_destination: Vec<String>,
    pub size: u64,
    pub state: OperationState,
    /// Why it was skipped or failed, or why the last Undo or Redo of it
    /// didn't happen. Not a Reason, which says why the Plan proposed it.
    pub note: Option<String>,
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

/// One Operation as its Journal records tell it.
pub(crate) struct JournaledOperation {
    pub id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub planned: SourceState,
    pub state: OperationState,
    /// Why it wasn't done, as journaled in its last outcome's `reason`.
    pub note: Option<String>,
    /// Set while the last intent for it has no outcome: that pass was cut
    /// short by a crash, maybe after the item moved.
    pub interrupted: Option<Phase>,
}

impl JournaledOperation {
    /// The record written before `phase` touches the item.
    pub fn intent(&self, phase: Phase) -> Record {
        match phase {
            Phase::Run => Record::Operation {
                operation_id: self.id.clone(),
                kind: self.kind,
                source: self.source.clone(),
                destination: self.destination.clone(),
                planned: self.planned,
            },
            Phase::Undo | Phase::Redo => Record::Intent { operation_id: self.id.clone(), phase },
        }
    }

    /// Where `phase` moves the item from, and to.
    pub fn route(&self, phase: Phase) -> (&Path, &Path) {
        match phase {
            Phase::Run | Phase::Redo => (&self.source, &self.destination),
            Phase::Undo => (&self.destination, &self.source),
        }
    }

    pub fn outcome(&self, note: Option<String>) -> Outcome {
        Outcome {
            operation_id: self.id.clone(),
            kind: self.kind,
            source: self.source.clone(),
            destination: self.destination.clone(),
            note,
        }
    }
}

/// The state an Operation is in once `phase` ended with `outcome`.
fn next_state(state: OperationState, phase: Phase, outcome: OutcomeKind) -> OperationState {
    match (phase, outcome) {
        (Phase::Run | Phase::Redo, OutcomeKind::Done) => OperationState::Done,
        (Phase::Undo, OutcomeKind::Done) => OperationState::Undone,
        (Phase::Run, OutcomeKind::Skipped) => OperationState::Skipped,
        (_, OutcomeKind::Failed) => OperationState::Failed,
        (_, OutcomeKind::Unrecoverable) => OperationState::Unrecoverable,
        // A skipped Undo or Redo leaves the Operation as it was.
        (Phase::Undo | Phase::Redo, OutcomeKind::Skipped) => state,
    }
}

/// A Session's Journal, read.
pub(crate) struct SessionJournal {
    pub header: SessionHeader,
    pub finished: bool,
    /// In the order they ran.
    pub operations: Vec<JournaledOperation>,
}

impl SessionJournal {
    pub fn read(journal_dir: &Path, id: &str) -> Result<SessionJournal, SessionError> {
        let (header, records) = journal::read(journal_dir, id)?;
        let mut session = SessionJournal { header, finished: false, operations: Vec::new() };
        for record in &records {
            session.apply(record);
        }
        Ok(session)
    }

    /// Brings the Session up to date with `record`, as just read or written.
    pub fn apply(&mut self, record: &Record) {
        match record {
            Record::Operation { operation_id, kind, source, destination, planned } => {
                self.operations.push(JournaledOperation {
                    id: operation_id.clone(),
                    kind: *kind,
                    source: source.clone(),
                    destination: destination.clone(),
                    planned: *planned,
                    state: OperationState::Started,
                    note: None,
                    interrupted: Some(Phase::Run),
                });
            }
            Record::Intent { operation_id, phase } => {
                if let Some(op) = self.operation(operation_id) {
                    op.interrupted = Some(*phase);
                }
            }
            Record::Outcome { operation_id, phase, outcome, reason: note } => {
                if let Some(op) = self.operation(operation_id) {
                    op.state = next_state(op.state, *phase, *outcome);
                    op.note = note.clone();
                    op.interrupted = None;
                }
            }
            Record::Finished => self.finished = true,
        }
    }

    /// The Operation `id`; a record about any other is ignored.
    fn operation(&mut self, id: &str) -> Option<&mut JournaledOperation> {
        self.operations.iter_mut().rev().find(|op| op.id == id)
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
            total_size += op.planned.size;
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
            id: self.header.id.clone(),
            scan_root: self.header.scan_root.clone(),
            trash_destination: self.header.trash_destination,
            started_at: self.header.started_at,
            status,
            done,
            restored,
            left_to_undo,
            total_size,
            kind_counts,
        }
    }

    /// The Operations in `state`, limited to `only` if given.
    pub fn in_state<'a>(
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
    ids.sort_unstable_by(|a, b| b.cmp(a));
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

/// Session `session_id` and each of its Operations, from its Journal.
pub fn session_details(session_id: &str, journal_dir: &Path) -> Result<SessionDetails, SessionError> {
    let session = SessionJournal::read(journal_dir, session_id)?;
    let root = &session.header.scan_root;
    let operations = session
        .operations
        .iter()
        .map(|op| SessionOperation {
            id: op.id.clone(),
            kind: op.kind,
            source: op.source.clone(),
            destination: op.destination.clone(),
            relative_path: names_below(root, &op.source),
            relative_destination: names_below(root, &op.destination),
            size: op.planned.size,
            state: op.state,
            note: op.note.clone(),
        })
        .collect();
    Ok(SessionDetails { info: session.info(), operations })
}

/// The names from `root` down to `path`, its own name last. Every path a
/// Session journals is inside its Scan Root; one that somehow isn't is
/// given in full.
fn names_below(root: &Path, path: &Path) -> Vec<String> {
    let below = path.strip_prefix(root).unwrap_or(path);
    below
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect()
}
