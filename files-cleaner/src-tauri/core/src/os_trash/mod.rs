//! Nothing here ever deletes an item for good.

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

pub const TRASH_PLACE: &str = os::PLACE;

pub trait OsTrash {
    /// `None` doesn't promise `trash` will work.
    fn blocked(&self, folder: &Path) -> Option<String>;

    /// A refusal, never a permanent delete.
    fn trash(&self, path: &Path) -> Result<PathBuf, String>;

    fn put_back(&self, landed: &Path, to: &Path) -> Result<(), String>;

    /// For records the OS keeps apart from the item, left over once it has
    /// moved back.
    fn forget(&self, _landed: &Path) {}
}

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
