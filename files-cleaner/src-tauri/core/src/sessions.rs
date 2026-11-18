//! Past Sessions for the History screens: the list, one Session's details, deleting them all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::journal::{self, lock_sessions, Journals, SessionError};
use crate::planner::{names_below, OperationKind};
use crate::session_journal::{OperationState, SessionJournal};

/// One past Session, for the Session list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub scan_root: PathBuf,
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
    /// Where the Operation put it; `None` for a Trash that hasn't landed.
    pub destination: Option<PathBuf>,
    /// Whether `destination` is in the System Trash. An older Session's
    /// Trash isn't: it went to a folder inside its Scan Root.
    pub in_system_trash: bool,
    /// The names from the Scan Root down to the source, its own name last.
    pub relative_path: Vec<String>,
    pub size: u64,
    pub state: OperationState,
    /// Why it was skipped or failed, or why the last Undo or Redo of it
    /// didn't happen. Not a Reason, which says why the Plan proposed it.
    pub note: Option<String>,
}

impl SessionOperation {
    /// Where the item is now, for opening it: `None` while it's in the
    /// System Trash, which is the OS's to show, or once it's gone for good.
    pub fn where_now(&self) -> Option<&Path> {
        match self.state {
            OperationState::Done if !self.in_system_trash => self.destination.as_deref(),
            OperationState::Done | OperationState::Unrecoverable => None,
            _ => Some(&self.source),
        }
    }
}

/// Sums up `session` for the Session list.
fn session_info(session: &SessionJournal) -> SessionInfo {
    let carried_out = session.operations.iter().filter(|op| {
        matches!(
            op.state,
            OperationState::Done | OperationState::Undone | OperationState::Unrecoverable
        )
    });
    let mut done = 0;
    let mut total_size = 0;
    let mut kind_counts = BTreeMap::new();
    for op in carried_out {
        done += 1;
        total_size += op.planned.size;
        *kind_counts.entry(op.kind).or_insert(0) += 1;
    }
    let in_state = |state| {
        session
            .operations
            .iter()
            .filter(|op| op.state == state)
            .count()
    };
    let restored = in_state(OperationState::Undone);
    let left_to_undo = in_state(OperationState::Done);
    let status = if restored > 0 && restored == done {
        SessionStatus::Undone
    } else if restored > 0 {
        SessionStatus::PartlyUndone
    } else if session.finished {
        SessionStatus::Completed
    } else {
        SessionStatus::Crashed
    };
    SessionInfo {
        id: session.header.id.clone(),
        scan_root: session.header.scan_root.clone(),
        started_at: session.header.started_at,
        status,
        done,
        restored,
        left_to_undo,
        total_size,
        kind_counts,
    }
}

/// The Sessions journaled in `journal_dir`, newest first, skipping
/// `offset` and returning at most `limit`, plus how many there are in all.
/// Session IDs sort by start time, so only the Journals on the page are
/// read. A Journal that can't be understood (say, one with a record an
/// older version can't read) is left out, so it doesn't hide the others;
/// its page is one short.
pub fn list_sessions(
    journal_dir: &Path,
    offset: usize,
    limit: usize,
) -> Result<SessionPage, SessionError> {
    if !journal_dir.exists() {
        return Ok(SessionPage {
            total: 0,
            sessions: Vec::new(),
        });
    }
    let journals = Journals::open(journal_dir)?;
    let mut ids = journals.session_ids()?;
    ids.sort_unstable_by(|a, b| b.cmp(a));
    let mut sessions = Vec::new();
    for id in ids.iter().skip(offset).take(limit) {
        match SessionJournal::read(&journals, id) {
            Ok(session) => sessions.push(session_info(&session)),
            Err(SessionError::BadJournal { .. }) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(SessionPage {
        total: ids.len(),
        sessions,
    })
}

/// Session `session_id` and each of its Operations, from its Journal.
pub fn session_details(
    session_id: &str,
    journal_dir: &Path,
) -> Result<SessionDetails, SessionError> {
    let session = SessionJournal::read(&Journals::open(journal_dir)?, session_id)?;
    let root = &session.header.scan_root;
    let operations = session
        .operations
        .iter()
        .map(|op| SessionOperation {
            id: op.id.clone(),
            kind: op.kind,
            source: op.source.clone(),
            destination: op.destination.clone(),
            in_system_trash: op.in_system_trash,
            relative_path: names_below(root, &op.source),
            size: op.planned.size,
            state: op.state,
            note: op.note.clone(),
        })
        .collect();
    Ok(SessionDetails {
        info: session_info(&session),
        operations,
    })
}

/// Deletes every Session's Journal in `journal_dir`, interrupted or
/// damaged ones too, and returns how many there were. Nothing else is
/// touched: not a Scan Root, the System Trash, or anything a Journal
/// names. Fails with `SessionError::Busy` during a Run, Undo or Redo.
pub fn delete_all_sessions(journal_dir: &Path) -> Result<usize, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    Ok(journal::remove_all_sessions(journal_dir)?)
}
