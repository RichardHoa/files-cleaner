//! Committed record by record, so a Session can be undone after a crash
//! (ADR-0012).

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use rusqlite::{params, Connection, ErrorCode, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::database::{delete_database, is_damaged, open_database, Durability};
use crate::planner::{OperationKind, SourceState};
use crate::scanner::millis;
use crate::session_id;

const DATABASE_FILE_NAME: &str = "journal.sqlite";
/// One file per Session, before ADR-0012.
const OLD_EXTENSION: &str = "jsonl";
const LOCK_FILE_NAME: &str = "sessions.lock";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHeader {
    pub id: String,
    pub scan_root: PathBuf,
    /// Milliseconds since the Unix epoch.
    pub started_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Run,
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutcomeKind {
    Done,
    /// Nothing is wrong with the Operation: it can be tried again later.
    Skipped,
    Failed,
    /// The trashed copy is gone, or its location was never recorded.
    Unrecoverable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "camelCase")]
pub enum Record {
    /// Before the item is touched, so a crash midway shows.
    #[serde(rename_all = "camelCase")]
    Operation {
        operation_id: String,
        kind: OperationKind,
        source: PathBuf,
        /// Only older Journals have a Trash's here; it isn't known until it has
        /// happened, so it comes with the outcome.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination: Option<PathBuf>,
        /// Redo checks the item against it.
        planned: SourceState,
    },
    /// Before the item is touched, so a crash midway shows.
    #[serde(rename_all = "camelCase")]
    Intent {
        operation_id: String,
        phase: Phase,
    },
    /// An intent with no outcome after it was cut short by a crash.
    #[serde(rename_all = "camelCase")]
    Outcome {
        operation_id: String,
        phase: Phase,
        outcome: OutcomeKind,
        reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        landed: Option<PathBuf>,
    },
    Finished,
}

#[derive(Debug)]
pub enum SessionError {
    Busy,
    UnknownSession(String),
    BadJournal { session: String, problem: String },
    Io(io::Error),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SessionError::Busy => f.write_str(
                "Another Session or Undo is in progress; try again once it has finished",
            ),
            SessionError::UnknownSession(id) => write!(f, "There is no Session {id}"),
            SessionError::BadJournal { session, problem } => {
                write!(f, "The Journal of Session {session} is damaged: {problem}")
            }
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

/// The OS releases it if the app crashes, so a crash never leaves it locked.
pub struct SessionLock {
    _file: File,
}

pub fn lock_sessions(journal_dir: &Path) -> Result<SessionLock, SessionError> {
    fs::create_dir_all(journal_dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(journal_dir.join(LOCK_FILE_NAME))?;
    match file.try_lock() {
        Ok(()) => Ok(SessionLock { _file: file }),
        Err(TryLockError::WouldBlock) => Err(SessionError::Busy),
        Err(TryLockError::Error(e)) => Err(e.into()),
    }
}

pub struct Journal {
    journals: Journals,
    id: String,
}

impl Journal {
    pub fn create(journal_dir: &Path, scan_root: PathBuf) -> io::Result<(SessionHeader, Journal)> {
        let journals = Journals::open(journal_dir)?;
        let now = SystemTime::now();
        let root = utf8(&scan_root)?.to_owned();
        for id in session_id::candidates(now) {
            let started_at = millis(now);
            let inserted = journals.db.execute(
                "INSERT INTO sessions (id, scan_root, started_at) VALUES (?1, ?2, ?3)",
                params![id, root, started_at],
            );
            match inserted {
                Ok(_) => {}
                Err(e) if e.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) => continue,
                Err(e) => return Err(sql(e)),
            }
            let header = SessionHeader {
                id: id.clone(),
                scan_root,
                started_at,
            };
            return Ok((header, Journal { journals, id }));
        }
        unreachable!("there is always another Session ID to try")
    }

    pub fn append(&mut self, record: &Record) -> io::Result<()> {
        let json = serde_json::to_string(record).expect("records always serialise");
        self.journals
            .db
            .execute(
                "INSERT INTO records (session_id, record) VALUES (?1, ?2)",
                params![self.id, json],
            )
            .map(|_| ())
            .map_err(sql)
    }

    pub fn journals(&self) -> &Journals {
        &self.journals
    }

    pub fn discard(mut self) -> io::Result<()> {
        self.journals.remove(&self.id)
    }
}

/// Kept open, so a pass over many Sessions opens the database once.
pub struct Journals {
    db: Connection,
}

impl Journals {
    pub fn open(journal_dir: &Path) -> io::Result<Journals> {
        fs::create_dir_all(journal_dir)?;
        let mut db = open_database(
            &journal_dir.join(DATABASE_FILE_NAME),
            Durability::Full,
            MIGRATIONS,
        )
        .map_err(sql)?;
        import_old_journals(&mut db, journal_dir);
        Ok(Journals { db })
    }

    pub fn reopen(self, id: &str) -> io::Result<Journal> {
        if header(&self.db, id).map_err(sql)?.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("There is no Session {id}"),
            ));
        }
        Ok(Journal {
            journals: self,
            id: id.to_owned(),
        })
    }

    pub fn read(&self, id: &str) -> Result<(SessionHeader, Vec<Record>), SessionError> {
        if !session_id::is_valid(id) {
            return Err(SessionError::UnknownSession(id.to_string()));
        }
        let Some(header) = header(&self.db, id).map_err(sql)? else {
            return Err(SessionError::UnknownSession(id.to_string()));
        };
        let mut query = self
            .db
            .prepare("SELECT seq, record FROM records WHERE session_id = ?1 ORDER BY seq")
            .map_err(sql)?;
        let rows = query
            .query_map([id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql)?;
        let mut records = Vec::new();
        for row in rows {
            let (seq, json) = row.map_err(sql)?;
            let record = serde_json::from_str(&json).map_err(|e| SessionError::BadJournal {
                session: id.to_string(),
                problem: format!("record {seq}: {e}"),
            })?;
            records.push(record);
        }
        Ok((header, records))
    }

    pub fn session_ids(&self) -> io::Result<Vec<String>> {
        let mut query = self.db.prepare("SELECT id FROM sessions").map_err(sql)?;
        let ids = query
            .query_map([], |row| row.get(0))
            .map_err(sql)?
            .collect::<Result<_, _>>()
            .map_err(sql)?;
        Ok(ids)
    }

    pub fn remove(&mut self, id: &str) -> io::Result<()> {
        let tx = self.db.transaction().map_err(sql)?;
        tx.execute("DELETE FROM records WHERE session_id = ?1", [id])
            .map_err(sql)?;
        tx.execute("DELETE FROM sessions WHERE id = ?1", [id])
            .map_err(sql)?;
        tx.commit().map_err(sql)
    }

    /// Old Journal files that couldn't be imported go too, or they would
    /// outlive "delete all".
    pub fn remove_all(&mut self, journal_dir: &Path) -> io::Result<usize> {
        let tx = self.db.transaction().map_err(sql)?;
        tx.execute("DELETE FROM records", []).map_err(sql)?;
        let removed = tx.execute("DELETE FROM sessions", []).map_err(sql)?;
        tx.commit().map_err(sql)?;
        remove_old_journals(journal_dir)?;
        Ok(removed)
    }
}

/// A database too damaged to empty is deleted whole, so the Journal can always
/// start again.
pub fn remove_all_sessions(journal_dir: &Path) -> io::Result<usize> {
    // Closed before it is deleted.
    let removed =
        Journals::open(journal_dir).and_then(|mut journals| journals.remove_all(journal_dir));
    match removed {
        Ok(removed) => Ok(removed),
        Err(e) if damaged(&e) => {
            delete_database(&journal_dir.join(DATABASE_FILE_NAME))?;
            remove_old_journals(journal_dir)?;
            Ok(0)
        }
        Err(e) => Err(e),
    }
}

fn damaged(e: &io::Error) -> bool {
    e.get_ref()
        .and_then(|e| e.downcast_ref::<rusqlite::Error>())
        .is_some_and(is_damaged)
}

fn remove_old_journals(journal_dir: &Path) -> io::Result<()> {
    for path in old_journals(journal_dir)? {
        remove_if_there(&path)?;
    }
    Ok(())
}

const MIGRATIONS: &[&str] = &["CREATE TABLE sessions (
         id TEXT PRIMARY KEY,
         scan_root TEXT NOT NULL,
         -- Milliseconds since the Unix epoch.
         started_at INTEGER NOT NULL
     ) STRICT;
     -- Every record after a Session's header, as JSON, in the order written.
     CREATE TABLE records (
         seq INTEGER PRIMARY KEY,
         session_id TEXT NOT NULL REFERENCES sessions (id),
         record TEXT NOT NULL
     ) STRICT;
     CREATE INDEX records_by_session ON records (session_id, seq);"];

/// So unreadable old files aren't read again on every open.
static UNREADABLE: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

/// So Sessions from before ADR-0012 can still be undone. A file goes only once
/// its Session is committed, and one that fails is left for the next open, so
/// it never stops the Journal being used.
fn import_old_journals(db: &mut Connection, journal_dir: &Path) {
    let Ok(paths) = old_journals(journal_dir) else {
        return;
    };
    let mut unreadable = UNREADABLE.lock().unwrap_or_else(|e| e.into_inner());
    for path in paths {
        if unreadable.contains(&path) {
            continue;
        }
        // Perhaps only held open by another program for now.
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Some((header, records)) = read_old_journal(&text) else {
            unreadable.insert(path);
            continue;
        };
        if import_old_journal(db, &header, records).is_ok() {
            let _ = remove_if_there(&path);
        }
    }
}

fn import_old_journal(
    db: &mut Connection,
    header: &SessionHeader,
    records: Vec<String>,
) -> io::Result<()> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    // Already there if a crash came between the commit and the delete.
    let inserted = tx
        .execute(
            "INSERT OR IGNORE INTO sessions (id, scan_root, started_at) VALUES (?1, ?2, ?3)",
            params![header.id, utf8(&header.scan_root)?, header.started_at],
        )
        .map_err(sql)?;
    if inserted == 1 {
        for record in records {
            tx.execute(
                "INSERT INTO records (session_id, record) VALUES (?1, ?2)",
                params![header.id, record],
            )
            .map_err(sql)?;
        }
    }
    tx.commit().map_err(sql)
}

fn old_journals(journal_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for child in fs::read_dir(journal_dir)? {
        let path = child?.path();
        if path.extension().is_some_and(|e| e == OLD_EXTENSION) {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn remove_if_there(path: &Path) -> io::Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// A last line cut short by a crash is ignored.
fn read_old_journal(text: &str) -> Option<(SessionHeader, Vec<String>)> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let (first, rest) = lines.split_first()?;
    let header: SessionHeader = serde_json::from_str(first).ok()?;
    if !session_id::is_valid(&header.id) {
        return None;
    }
    let mut records = Vec::with_capacity(rest.len());
    for (i, line) in rest.iter().enumerate() {
        match serde_json::from_str::<Record>(line) {
            Ok(_) => records.push((*line).to_owned()),
            Err(_) if i + 1 == rest.len() && !text.ends_with('\n') => {}
            Err(_) => return None,
        }
    }
    Some((header, records))
}

fn header(db: &Connection, id: &str) -> rusqlite::Result<Option<SessionHeader>> {
    db.query_row(
        "SELECT scan_root, started_at FROM sessions WHERE id = ?1",
        [id],
        |row| {
            Ok(SessionHeader {
                id: id.to_owned(),
                scan_root: PathBuf::from(row.get::<_, String>(0)?),
                started_at: row.get(1)?,
            })
        },
    )
    .optional()
}

fn utf8(path: &Path) -> io::Result<&str> {
    path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} isn't valid Unicode", path.display()),
        )
    })
}

fn sql(e: rusqlite::Error) -> io::Error {
    io::Error::other(e)
}
