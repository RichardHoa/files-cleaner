//! The Journal: one append-only JSON-lines file per Session, flushed to
//! disk after every record, so a Session can be undone after a crash or a
//! restart (ADR-0002). Also the lock that lets one Session run at a time.

use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::TrashDestination;
use crate::executor::SourceState;
use crate::planner::OperationKind;

const EXTENSION: &str = "jsonl";
const LOCK_FILE_NAME: &str = "sessions.lock";

/// One line of a Journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "camelCase")]
pub enum Record {
    /// Always the first line.
    #[serde(rename_all = "camelCase")]
    Session { id: String, scan_root: PathBuf, trash_destination: TrashDestination, started_at: i64 },
    /// Written before the Operation touches the disk, with everything Undo
    /// needs to reverse it.
    #[serde(rename_all = "camelCase")]
    Intent {
        operation_id: String,
        kind: OperationKind,
        source: PathBuf,
        destination: PathBuf,
        /// The source as planned, which Redo checks it against. Missing
        /// from Journals written before Redo existed.
        #[serde(default)]
        planned: Option<SourceState>,
    },
    #[serde(rename_all = "camelCase")]
    Done { operation_id: String },
    #[serde(rename_all = "camelCase")]
    Skipped { operation_id: String, reason: String },
    #[serde(rename_all = "camelCase")]
    Failed { operation_id: String, reason: String },
    /// The last Operation has been recorded.
    Finished,
    UndoStarted,
    /// Written before Undo reverses the Operation.
    #[serde(rename_all = "camelCase")]
    UndoIntent { operation_id: String },
    #[serde(rename_all = "camelCase")]
    Restored { operation_id: String },
    #[serde(rename_all = "camelCase")]
    UndoSkipped { operation_id: String, reason: String },
    #[serde(rename_all = "camelCase")]
    Unrecoverable { operation_id: String, reason: String },
    UndoFinished,
    /// Written before Redo carries the Operation out again.
    #[serde(rename_all = "camelCase")]
    RedoIntent { operation_id: String },
    #[serde(rename_all = "camelCase")]
    Redone { operation_id: String },
    #[serde(rename_all = "camelCase")]
    RedoSkipped { operation_id: String, reason: String },
}

#[derive(Debug)]
pub enum SessionError {
    /// Another Session, or an Undo, is in progress.
    Busy,
    UnknownSession(String),
    /// A Journal that can't be understood.
    BadJournal { path: PathBuf, problem: String },
    Io(io::Error),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SessionError::Busy => f.write_str("Another Session or Undo is in progress; try again once it has finished"),
            SessionError::UnknownSession(id) => write!(f, "There is no Session {id}"),
            SessionError::BadJournal { path, problem } => write!(f, "The Journal {} is damaged: {problem}", path.display()),
            SessionError::Io(e) => write!(f, "Couldn't write the Journal: {e}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<io::Error> for SessionError {
    fn from(e: io::Error) -> Self {
        SessionError::Io(e)
    }
}

/// Held while a Session or an Undo runs; dropping it lets the next start.
/// The OS releases it if the app crashes.
pub struct SessionLock {
    _file: File,
}

/// Takes the lock that lets one Session or Undo use `journal_dir` at a
/// time, or fails with `SessionError::Busy`. Also works across processes.
pub fn lock_sessions(journal_dir: &Path) -> Result<SessionLock, SessionError> {
    fs::create_dir_all(journal_dir)?;
    let file = OpenOptions::new().create(true).truncate(false).write(true).open(journal_dir.join(LOCK_FILE_NAME))?;
    match file.try_lock() {
        Ok(()) => Ok(SessionLock { _file: file }),
        Err(TryLockError::WouldBlock) => Err(SessionError::Busy),
        Err(TryLockError::Error(e)) => Err(e.into()),
    }
}

/// An open Journal that records are appended to.
pub struct Journal {
    file: File,
}

impl Journal {
    /// Starts the Journal for a new Session; fails if `id` is taken.
    pub fn create(journal_dir: &Path, id: &str) -> io::Result<Journal> {
        let file = OpenOptions::new().create_new(true).append(true).open(path(journal_dir, id))?;
        sync_folder(journal_dir)?;
        Ok(Journal { file })
    }

    /// Reopens an existing Session's Journal to add to it.
    pub fn reopen(journal_dir: &Path, id: &str) -> io::Result<Journal> {
        let file = OpenOptions::new().append(true).open(path(journal_dir, id))?;
        Ok(Journal { file })
    }

    /// Writes `record` and waits until it is on disk.
    pub fn append(&mut self, record: &Record) -> io::Result<()> {
        let mut line = serde_json::to_vec(record).expect("records always serialise");
        line.push(b'\n');
        self.file.write_all(&line)?;
        self.file.sync_data()
    }
}

/// Every record of Session `id`'s Journal. A last line cut short by a
/// crash is ignored.
pub fn read(journal_dir: &Path, id: &str) -> Result<Vec<Record>, SessionError> {
    // IDs come from the UI; only the app's own ID shape can name a Journal.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(SessionError::UnknownSession(id.to_string()));
    }
    let path = path(journal_dir, id);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(SessionError::UnknownSession(id.to_string())),
        Err(e) => return Err(e.into()),
    };
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut records = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        match serde_json::from_str(line) {
            Ok(record) => records.push(record),
            Err(_) if i + 1 == lines.len() && !text.ends_with('\n') => {}
            Err(e) => return Err(SessionError::BadJournal { path, problem: format!("line {}: {e}", i + 1) }),
        }
    }
    if !matches!(records.first(), Some(Record::Session { .. })) {
        return Err(SessionError::BadJournal { path, problem: "it doesn't start with a Session record".into() });
    }
    Ok(records)
}

/// The IDs of every Session with a Journal in `journal_dir`.
pub fn session_ids(journal_dir: &Path) -> io::Result<Vec<String>> {
    let children = match fs::read_dir(journal_dir) {
        Ok(children) => children,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut ids = Vec::new();
    for child in children {
        let path = child?.path();
        if path.extension().is_some_and(|e| e == EXTENSION) {
            if let Some(stem) = path.file_stem() {
                ids.push(stem.to_string_lossy().into_owned());
            }
        }
    }
    Ok(ids)
}

/// Waits until a new file's name in `folder` is on disk, not just its
/// contents. Windows can't open a folder to sync it, and doesn't need to.
fn sync_folder(folder: &Path) -> io::Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    File::open(folder)?.sync_all()
}

fn path(journal_dir: &Path, id: &str) -> PathBuf {
    journal_dir.join(format!("{id}.{EXTENSION}"))
}
