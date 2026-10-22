//! Cards and their embeddings, kept on disk so a file is read by the models
//! once: keyed by its content hash, so renaming or moving it still finds
//! them, and by both models' IDs, so changing either model leaves the old
//! entries unused. The answers to placing questions are kept here too.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::card::Card;
use crate::cluster::Vector;
use crate::place::Answer;

/// A folder of stored cards, in the app's data folder.
pub struct CardStore {
    folder: PathBuf,
}

/// One file's card, and its embedding once it has one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stored {
    pub card: Card,
    /// A PDF with no text that OCR couldn't read either; it has an empty card.
    #[serde(default)]
    pub scanned: bool,
    pub embedding: Option<Vector>,
}

/// The models a store entry was made with.
pub struct ModelIds<'a> {
    pub card: &'a str,
    pub embedding: &'a str,
}

impl CardStore {
    /// The store in `folder`, which is created when the first card is stored.
    pub fn open(folder: impl Into<PathBuf>) -> Self {
        CardStore { folder: folder.into() }
    }

    /// Forgets every stored card and answer, so every file is read again.
    pub fn clear(&self) -> std::io::Result<()> {
        match fs::remove_dir_all(&self.folder) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    pub(crate) fn get(&self, models: &ModelIds, hash: &blake3::Hash) -> Option<Stored> {
        let bytes = fs::read(self.path(models, hash)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Stores `entry`. A store that can't be written is only slower next
    /// time, so failures are ignored.
    pub(crate) fn put(&self, models: &ModelIds, hash: &blake3::Hash, entry: &Stored) {
        write(&self.path(models, hash), entry);
    }

    /// The answer stored for a placing question, by the model `card_model`
    /// and the question's prompt.
    pub(crate) fn get_answer(&self, card_model: &str, prompt: &str) -> Option<Answer> {
        let bytes = fs::read(self.answer_path(card_model, prompt)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub(crate) fn put_answer(&self, card_model: &str, prompt: &str, answer: &Answer) {
        write(&self.answer_path(card_model, prompt), answer);
    }

    fn answer_path(&self, card_model: &str, prompt: &str) -> PathBuf {
        let model = blake3::hash(card_model.as_bytes());
        let question = blake3::hash(prompt.as_bytes());
        self.folder.join("answers").join(&model.to_hex()[..16]).join(format!("{}.json", question.to_hex()))
    }

    fn path(&self, models: &ModelIds, hash: &blake3::Hash) -> PathBuf {
        let pair = blake3::hash(format!("{}\n{}", models.card, models.embedding).as_bytes());
        self.folder.join(&pair.to_hex()[..16]).join(format!("{}.json", hash.to_hex()))
    }
}

/// Writes `value` to `path` through a temporary file and a rename, so a
/// crash never leaves half an entry. Failures are ignored.
fn write(path: &Path, value: &impl Serialize) {
    let Some(folder) = path.parent() else { return };
    let Ok(json) = serde_json::to_vec(value) else { return };
    let temporary = path.with_extension("tmp");
    let written = fs::create_dir_all(folder).and_then(|()| fs::write(&temporary, json));
    if written.and_then(|()| fs::rename(&temporary, path)).is_err() {
        let _ = fs::remove_file(&temporary);
    }
}

/// The BLAKE3 of the file at `path`, as the scan's duplicate check computes it.
pub(crate) fn content_hash(path: &Path) -> Option<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(fs::File::open(path).ok()?).ok()?;
    Some(hasher.finalize())
}
