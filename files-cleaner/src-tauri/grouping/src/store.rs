//! Cards and their embeddings, kept on disk so a file is read by the models
//! once: keyed by its content hash, so renaming or moving it still finds
//! them, and by both models' IDs, so changing either model leaves the old
//! entries unused. The answers to placing questions are kept here too. They
//! live in one SQLite database (ADR-0012); it is only a cache, so anything
//! that can't be read or written there just means a file is read again.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use files_cleaner_core::{delete_database, is_damaged, open_database, Durability};
use rusqlite::{params, Connection, ErrorCode, OptionalExtension};

use crate::card::Card;
use crate::cluster::Vector;
use crate::place::Answer;

const DATABASE_FILE_NAME: &str = "cards.sqlite";

/// A folder of stored cards, in the app's data folder.
pub struct CardStore {
    folder: PathBuf,
    db: Mutex<Database>,
}

enum Database {
    /// Opened when first used.
    Unopened,
    Open(Connection),
    /// Couldn't be opened, so the store goes without until it is cleared.
    Unavailable,
}

/// One file's card, and its embedding once it has one.
#[derive(Debug, Clone)]
pub struct Stored {
    pub card: Card,
    /// A PDF with no text that OCR couldn't read either; it has an empty card.
    pub scanned: bool,
    pub embedding: Option<Vector>,
}

/// The models a store entry was made with.
pub struct ModelIds<'a> {
    pub card: &'a str,
    pub embedding: &'a str,
}

impl CardStore {
    /// The store in `folder`, which is created when the store is first used.
    pub fn open(folder: impl Into<PathBuf>) -> Self {
        CardStore {
            folder: folder.into(),
            db: Mutex::new(Database::Unopened),
        }
    }

    /// Forgets every stored card and answer, so every file is read again.
    /// A database too damaged to open or read is deleted instead, so the
    /// next use starts a new one. Fails while another analysis is writing.
    pub fn clear(&self) -> io::Result<()> {
        let mut db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*db, Database::Unavailable) {
            *db = Database::Unopened;
        }
        let cleared = connection(&mut db, &self.folder).and_then(|db| {
            let tx = db.unchecked_transaction()?;
            tx.execute("DELETE FROM cards", [])?;
            tx.execute("DELETE FROM answers", [])?;
            tx.commit()?;
            // Gives the space back; another analysis reading meanwhile only
            // keeps it for later.
            let _ = db.execute_batch("VACUUM");
            Ok(())
        });
        match cleared {
            Ok(()) => Ok(()),
            Err(e) if is_damaged(&e) => {
                *db = Database::Unopened;
                delete_database(&self.folder.join(DATABASE_FILE_NAME))
            }
            Err(e) => Err(io::Error::other(e)),
        }
    }

    pub(crate) fn get(&self, models: &ModelIds, hash: &blake3::Hash) -> Option<Stored> {
        self.with(|db| {
            db.query_row(
                "SELECT topic, kind, summary, scanned, embedding FROM cards
                 WHERE card_model = ?1 AND embedding_model = ?2 AND hash = ?3",
                params![models.card, models.embedding, hash.as_bytes()],
                |row| {
                    Ok(Stored {
                        card: Card {
                            topic: row.get(0)?,
                            kind: row.get(1)?,
                            summary: row.get(2)?,
                        },
                        scanned: row.get(3)?,
                        embedding: row.get::<_, Option<Vec<u8>>>(4)?.map(|b| from_bytes(&b)),
                    })
                },
            )
            .optional()
        })
        .and_then(|found| found)
        .ok()
        .flatten()
    }

    /// Stores `entry`. A store that can't be written is only slower next
    /// time, so failures are ignored.
    pub(crate) fn put(&self, models: &ModelIds, hash: &blake3::Hash, entry: &Stored) {
        let _ = self.with(|db| {
            db.execute(
                "INSERT OR REPLACE INTO cards
                 (card_model, embedding_model, hash, topic, kind, summary, scanned, embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    models.card,
                    models.embedding,
                    hash.as_bytes(),
                    entry.card.topic,
                    entry.card.kind,
                    entry.card.summary,
                    entry.scanned,
                    entry.embedding.as_deref().map(to_bytes),
                ],
            )
        });
    }

    /// The answer stored for a placing question, by the model `card_model`
    /// and the question's prompt.
    pub(crate) fn get_answer(&self, card_model: &str, prompt: &str) -> Option<Answer> {
        let question = blake3::hash(prompt.as_bytes());
        self.with(|db| {
            db.query_row(
                "SELECT folder FROM answers WHERE card_model = ?1 AND question = ?2",
                params![card_model, question.as_bytes()],
                |row| Ok(Answer { folder: row.get(0)? }),
            )
            .optional()
        })
        .and_then(|found| found)
        .ok()
        .flatten()
    }

    pub(crate) fn put_answer(&self, card_model: &str, prompt: &str, answer: &Answer) {
        let question = blake3::hash(prompt.as_bytes());
        let _ = self.with(|db| {
            db.execute(
                "INSERT OR REPLACE INTO answers (card_model, question, folder) VALUES (?1, ?2, ?3)",
                params![card_model, question.as_bytes(), answer.folder],
            )
        });
    }

    /// Runs `f` on the database, opening it first if it isn't open yet.
    fn with<T>(&self, f: impl FnOnce(&Connection) -> T) -> rusqlite::Result<T> {
        let mut db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        connection(&mut db, &self.folder).map(f)
    }
}

/// The open database, opening it in `folder` first if it isn't open yet.
/// One that couldn't be opened isn't tried again, unless it was only busy.
fn connection<'a>(db: &'a mut Database, folder: &Path) -> rusqlite::Result<&'a Connection> {
    if let Database::Unopened = db {
        *db = match open(folder) {
            Ok(opened) => Database::Open(opened),
            Err(e) => {
                if !is_busy(&e) {
                    *db = Database::Unavailable;
                }
                return Err(e);
            }
        };
    }
    match db {
        Database::Open(opened) => Ok(opened),
        _ => Err(rusqlite::Error::InvalidPath(folder.join(DATABASE_FILE_NAME))),
    }
}

/// The database in `folder`, created or brought up to date. Creating it
/// deletes the folders of JSON files the store was kept in before ADR-0012.
fn open(folder: &Path) -> rusqlite::Result<Connection> {
    let path = folder.join(DATABASE_FILE_NAME);
    let _ = fs::create_dir_all(folder);
    if !path.exists() {
        if let Ok(children) = fs::read_dir(folder) {
            for child in children.flatten() {
                if child.file_type().is_ok_and(|t| t.is_dir()) {
                    let _ = fs::remove_dir_all(child.path());
                }
            }
        }
    }
    // A cache: a power cut may lose the last few entries, never corrupt it.
    open_database(&path, Durability::Normal, MIGRATIONS)
}

/// Whether `e` is another connection holding the database for longer
/// than the busy timeout.
fn is_busy(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

/// The database's layout: see `open_database`.
const MIGRATIONS: &[&str] = &["CREATE TABLE cards (
         card_model TEXT NOT NULL,
         embedding_model TEXT NOT NULL,
         -- The file's BLAKE3.
         hash BLOB NOT NULL,
         topic TEXT NOT NULL,
         kind TEXT NOT NULL,
         summary TEXT NOT NULL,
         scanned INTEGER NOT NULL,
         -- Little-endian f32s.
         embedding BLOB,
         PRIMARY KEY (card_model, embedding_model, hash)
     ) STRICT;
     CREATE TABLE answers (
         card_model TEXT NOT NULL,
         -- The BLAKE3 of the placing prompt.
         question BLOB NOT NULL,
         folder TEXT,
         PRIMARY KEY (card_model, question)
     ) STRICT;"];

fn to_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_bytes(bytes: &[u8]) -> Vector {
    let (chunks, _) = bytes.as_chunks::<4>();
    chunks.iter().map(|b| f32::from_le_bytes(*b)).collect()
}

/// The BLAKE3 of the file at `path`, as the scan's duplicate check computes it.
pub(crate) fn content_hash(path: &Path) -> Option<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(fs::File::open(path).ok()?).ok()?;
    Some(hasher.finalize())
}
