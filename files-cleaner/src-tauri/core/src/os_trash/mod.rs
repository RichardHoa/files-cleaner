//! The OS Trash: sends items to the System Trash and puts them back. The
//! backend for each OS lives in a submodule; Run, Undo and Redo reach the OS
//! only through `OsTrash`, and nothing here ever deletes an item for good
//! (ADR-0001).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
use linux as os;
#[cfg(target_os = "macos")]
use macos as os;
#[cfg(target_os = "windows")]
use windows as os;

use std::path::{Path, PathBuf};

/// What this OS calls its System Trash, for Notes.
pub const TRASH_PLACE: &str = os::PLACE;

pub trait OsTrash {
    /// Why items in `folder` can't be trashed, if that can be told up front,
    /// without touching anything. `None` doesn't promise `trash` will work.
    fn blocked(&self, folder: &Path) -> Option<String>;

    /// Sends `path` to the System Trash and returns where it landed, or says
    /// why it can't. A trash never becomes a permanent delete.
    fn trash(&self, path: &Path) -> Result<PathBuf, String>;

    /// Moves the item at `landed` back to `to`, never replacing anything.
    fn put_back(&self, landed: &Path, to: &Path) -> Result<(), String>;

    /// Drops what the OS keeps about `landed`, an item that has already left
    /// the System Trash.
    fn forget(&self, _landed: &Path) {}
}

/// The real System Trash of the OS the app runs on.
pub struct SystemTrash;

impl OsTrash for SystemTrash {
    fn blocked(&self, folder: &Path) -> Option<String> {
        os::blocked(folder)
    }

    fn trash(&self, path: &Path) -> Result<PathBuf, String> {
        os::trash(path)
    }

    fn put_back(&self, landed: &Path, to: &Path) -> Result<(), String> {
        os::put_back(landed, to)
    }

    fn forget(&self, landed: &Path) {
        os::forget(landed);
    }
}
