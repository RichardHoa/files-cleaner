//! The Journal: one append-only JSON-lines file per Session, flushed to
//! disk after every record, so a Session can be undone after a crash or a
//! restart (ADR-0002). Also the lock that lets one Session run at a time.

use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::config::TrashDestination;
use crate::planner::{OperationKind, SourceState};
use crate::scanner::millis;
use crate::session_id;

const EXTENSION: &str = "jsonl";
const LOCK_FILE_NAME: &str = "sessions.lock";

/// The first line of a Journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHeader {
    pub id: String,
    pub scan_root: PathBuf,
    pub trash_destination: TrashDestination,
    /// Milliseconds since the Unix epoch.
    pub started_at: i64,
}

/// Which pass over a Session's Operations a record belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// The Session itself: source to destination.
    Run,
    /// Destination back to source.
    Undo,
    /// Source to destination again, after an Undo.
    Redo,
}

/// How one Operation's pass ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutcomeKind {
    /// The item moved.
    Done,
    /// Not moved, but nothing is wrong with the Operation: it can be tried
    /// again later.
    Skipped,
    /// The Run's move failed.
    Failed,
    /// Undo found the trashed copy gone.
    Unrecoverable,
}

/// Every line of a Journal after the header.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "camelCase")]
pub enum Record {
    /// Written before the Run touches the item, with everything Undo and
    /// Redo need: the Run's intent.
    #[serde(rename_all = "camelCase")]
    Operation {
        operation_id: String,
        kind: OperationKind,
        source: PathBuf,
        destination: PathBuf,
        /// The source as planned, which Redo checks it against.
        planned: SourceState,
    },
    /// Written before an Undo or a Redo touches the item.
    #[serde(rename_all = "camelCase")]
    Intent { operation_id: String, phase: Phase },
    /// How the last intent for the Operation ended. An intent with no
    /// outcome after it was cut short by a crash.
    #[serde(rename_all = "camelCase")]
    Outcome { operation_id: String, phase: Phase, outcome: OutcomeKind, reason: Option<String> },
    /// The Run has recorded its last Operation.
    Finished,
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
    /// Starts the Journal of a new Session of `scan_root`, starting now,
    /// and writes its header.
    pub fn create(
        journal_dir: &Path,
        scan_root: PathBuf,
        trash_destination: TrashDestination,
    ) -> io::Result<(SessionHeader, Journal)> {
        let now = SystemTime::now();
        for id in session_id::candidates(now) {
            let file = match OpenOptions::new().create_new(true).append(true).open(path(journal_dir, &id)) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            };
            sync_folder(journal_dir)?;
            let header = SessionHeader { id, scan_root, trash_destination, started_at: millis(now) };
            let mut journal = Journal { file };
            journal.append_line(&header)?;
            return Ok((header, journal));
        }
        unreachable!("there is always another Session ID to try")
    }

    /// Reopens an existing Session's Journal to add to it.
    pub fn reopen(journal_dir: &Path, id: &str) -> io::Result<Journal> {
        let file = OpenOptions::new().append(true).open(path(journal_dir, id))?;
        Ok(Journal { file })
    }

    /// Writes `record` and waits until it is on disk.
    pub fn append(&mut self, record: &Record) -> io::Result<()> {
        self.append_line(record)
    }

    fn append_line(&mut self, value: &impl Serialize) -> io::Result<()> {
        let mut line = serde_json::to_vec(value).expect("records always serialise");
        line.push(b'\n');
        self.file.write_all(&line)?;
        self.file.sync_data()
    }
}

/// Session `id`'s header and every record after it. A last line cut
/// short by a crash is ignored.
pub fn read(journal_dir: &Path, id: &str) -> Result<(SessionHeader, Vec<Record>), SessionError> {
    if !session_id::is_valid(id) {
        return Err(SessionError::UnknownSession(id.to_string()));
    }
    let path = path(journal_dir, id);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(SessionError::UnknownSession(id.to_string())),
        Err(e) => return Err(e.into()),
    };
    let bad = |problem: String| SessionError::BadJournal { path: path.clone(), problem };
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let Some((first, rest)) = lines.split_first() else {
        return Err(bad("it is empty".into()));
    };
    let header = serde_json::from_str(first).map_err(|e| bad(format!("its first line isn't a Session header: {e}")))?;
    let mut records = Vec::with_capacity(rest.len());
    for (i, line) in rest.iter().enumerate() {
        match serde_json::from_str(line) {
            Ok(record) => records.push(record),
            Err(_) if i + 1 == rest.len() && !text.ends_with('\n') => {}
            Err(e) => return Err(bad(format!("line {}: {e}", i + 2))),
        }
    }
    Ok((header, records))
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
