//! Cards and their embeddings, kept on disk so a file is read by the models
//! once: keyed by its content hash, so renaming or moving it still finds
//! them, and by both models' IDs, so changing either model leaves the old
//! entries unused.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::card::Card;
use crate::cluster::Vector;

/// A folder of stored cards, in the app's data folder.
pub struct CardStore {
    folder: PathBuf,
}

/// One file's card, and its embedding once it has one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stored {
    pub card: Card,
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

    pub(crate) fn get(&self, models: &ModelIds, hash: &blake3::Hash) -> Option<Stored> {
        let bytes = fs::read(self.path(models, hash)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Stores `entry`, written to a temporary file and then renamed, so a
    /// crash never leaves half an entry. A store that can't be written is
    /// only slower next time, so failures are ignored.
    pub(crate) fn put(&self, models: &ModelIds, hash: &blake3::Hash, entry: &Stored) {
        let path = self.path(models, hash);
        let Some(folder) = path.parent() else { return };
        let Ok(json) = serde_json::to_vec(entry) else { return };
        let temporary = path.with_extension("tmp");
        let written = fs::create_dir_all(folder).and_then(|()| fs::write(&temporary, json));
        if written.and_then(|()| fs::rename(&temporary, &path)).is_err() {
            let _ = fs::remove_file(&temporary);
        }
    }

    fn path(&self, models: &ModelIds, hash: &blake3::Hash) -> PathBuf {
        let pair = blake3::hash(format!("{}\n{}", models.card, models.embedding).as_bytes());
        self.folder.join(&pair.to_hex()[..16]).join(format!("{}.json", hash.to_hex()))
    }
}

/// The BLAKE3 of the file at `path`, as the scan's duplicate check computes it.
pub(crate) fn content_hash(path: &Path) -> Option<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(fs::File::open(path).ok()?).ok()?;
    Some(hasher.finalize())
}
