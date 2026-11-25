use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::journal::{self, lock_sessions, Journals, SessionError};
use crate::planner::{names_below, OperationKind};
use crate::session_journal::{OperationState, SessionJournal};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub scan_root: PathBuf,
    /// Milliseconds since the Unix epoch.
    pub started_at: i64,
    pub status: SessionStatus,
    /// Whether or not undone since.
    pub done: usize,
    pub restored: usize,
    /// Excludes those found gone by an earlier Undo.
    pub left_to_undo: usize,
    pub total_size: u64,
    pub kind_counts: BTreeMap<OperationKind, usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPage {
    /// Unreadable Journals count too, as only the requested page's are read.
    pub total: usize,
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    Completed,
    Crashed,
    Undone,
    PartlyUndone,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetails {
    pub info: SessionInfo,
    pub operations: Vec<SessionOperation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOperation {
    pub id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    /// False for an older Session's Trash, which went to a folder in its Scan
    /// Root.
    pub in_system_trash: bool,
    pub relative_path: Vec<String>,
    pub size: u64,
    pub state: OperationState,
    /// Not a Reason, which says why the Plan proposed it.
    pub note: Option<String>,
}

impl SessionOperation {
    /// `None` while it's in the System Trash, which is the OS's to show, or
    /// once it's gone.
    pub fn where_now(&self) -> Option<&Path> {
        match self.state {
            OperationState::Done if !self.in_system_trash => self.destination.as_deref(),
            OperationState::Done | OperationState::Unrecoverable => None,
            _ => Some(&self.source),
        }
    }
}

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

/// IDs sort by start time, so only the page's Journals are read. An unreadable
/// one is left out rather than hiding the others, so its page is one short.
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

/// Journals only: no Scan Root or Trash is touched.
pub fn delete_all_sessions(journal_dir: &Path) -> Result<usize, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    Ok(journal::remove_all_sessions(journal_dir)?)
}
