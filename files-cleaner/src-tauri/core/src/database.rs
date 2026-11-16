//! Opening the app's SQLite databases (ADR-0012): the Journal and the card
//! store, set up the same way.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, ErrorCode, TransactionBehavior};

/// How much of a database's last writes a power cut may lose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// None: every commit is on the disk itself before it returns, past the
    /// drive's own cache too.
    Full,
    /// The last few commits, but it is never left corrupt. For a cache.
    Normal,
}

/// The database at `path`, created if it isn't there, and brought up to
/// date by `migrations`: the layout each `user_version` adds to the one
/// before, oldest first.
pub fn open_database(
    path: &Path,
    durability: Durability,
    migrations: &[&str],
) -> rusqlite::Result<Connection> {
    let mut db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(5))?;
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
    migrate(&mut db, migrations)?;
    Ok(db)
}

/// Whether `e` says the database isn't one SQLite can read, or can't be
/// opened at all: one only deleting it fixes.
pub fn is_damaged(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase | ErrorCode::CannotOpen)
    )
}

/// Deletes the database at `path`, with the files SQLite keeps beside it.
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
    // Checked again holding the write lock, in case another connection
    // migrated it meanwhile.
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
