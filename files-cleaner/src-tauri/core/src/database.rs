//! ADR-0012: the Journal and the card store are set up the same way.

use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::{Connection, ErrorCode, TransactionBehavior};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// Nothing may be lost: the Journal must survive a power cut.
    Full,
    /// The last few commits may be lost: enough for a cache.
    Normal,
}

/// `migrations` run oldest first, each adding to the layout before it, so
/// released ones must never change.
pub fn open_database(
    path: &Path,
    durability: Durability,
    migrations: &[&str],
) -> rusqlite::Result<Connection> {
    let mut db = Connection::open(path)?;
    db.busy_timeout(BUSY_TIMEOUT)?;
    // Some locks SQLite takes here give up at once instead of waiting out the
    // busy timeout (seen on Windows, turning a new database to WAL). Each step
    // is safe to run again.
    let deadline = Instant::now() + BUSY_TIMEOUT;
    loop {
        match set_up(&mut db, durability, migrations) {
            Err(e)
                if e.sqlite_error_code() == Some(ErrorCode::DatabaseBusy)
                    && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            result => return result.map(|()| db),
        }
    }
}

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

fn set_up(
    db: &mut Connection,
    durability: Durability,
    migrations: &[&str],
) -> rusqlite::Result<()> {
    db.pragma_update(None, "journal_mode", "WAL")?;
    match durability {
        Durability::Full => {
            db.pragma_update(None, "synchronous", "FULL")?;
            // A plain fsync on Apple's systems leaves the writes in the
            // drive's cache; these use F_FULLFSYNC. Ignored elsewhere. It
            // makes each commit slower: chosen, for an Undo after a power cut.
            db.pragma_update(None, "fullfsync", true)?;
            db.pragma_update(None, "checkpoint_fullfsync", true)?;
        }
        Durability::Normal => db.pragma_update(None, "synchronous", "NORMAL")?,
    }
    migrate(db, migrations)
}

/// Damaged, as opposed to busy: only deleting it helps.
pub fn is_damaged(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase | ErrorCode::CannotOpen)
    )
}

/// SQLite's side files go too, or a new database would pick them up.
pub fn delete_database(path: &Path) -> io::Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.as_os_str().to_owned();
        file.push(suffix);
        match fs::remove_file(file) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn migrate(db: &mut Connection, migrations: &[&str]) -> rusqlite::Result<()> {
    let latest = migrations.len() as i64;
    if version(db)? >= latest {
        return Ok(());
    }
    // Another connection may have migrated it before we got the write lock.
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let from = version(&tx)?;
    for migration in migrations.iter().skip(from.max(0) as usize) {
        tx.execute_batch(migration)?;
    }
    if from < latest {
        tx.pragma_update(None, "user_version", latest)?;
    }
    tx.commit()
}

fn version(db: &Connection) -> rusqlite::Result<i64> {
    db.pragma_query_value(None, "user_version", |row| row.get(0))
}
