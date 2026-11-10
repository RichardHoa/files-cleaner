#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use files_cleaner_core::{
    self as core, load_config, scan, Config, DuplicateChoice, DuplicateProgress, FileChoice,
    FolderChoice, OsTrash, Plan, RunProgress, ScanResult, SessionError, SuggestedFolderChoice,
    SuggestedFolderError, Summary,
};
use tempfile::TempDir;

/// A temp folder for a test to scan.
///
/// It lives under the crate folder rather than the system temp dir: macOS
/// puts that under `/private`, and `target/` sits next to a `Cargo.toml`,
/// and both are Protected Paths that would hide the test's own files.
pub fn scan_root() -> TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(base)
        .unwrap()
}

/// `path` with symlinks resolved, in the form the app gives it: on Windows
/// `C:\…`, not the `\\?\C:\…` that `fs::canonicalize` returns.
pub fn real_path(path: &Path) -> PathBuf {
    let real = fs::canonicalize(path).unwrap();
    #[cfg(windows)]
    {
        if let Some(plain) = real
            .to_str()
            .and_then(|s| s.strip_prefix(r"\\?\"))
            .filter(|s| !s.starts_with(r"UNC\"))
        {
            return PathBuf::from(plain);
        }
    }
    real
}

/// Writes `contents` to `root/relative`, creating parent folders.
pub fn write(root: &Path, relative: &str, contents: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();
    path
}

pub fn config(text: &str, home: &Path) -> Config {
    load_config(text, home).unwrap()
}

pub fn empty_config(home: &Path) -> Config {
    config("{}", home)
}

/// Makes `link` a symlink to `target`. On Windows this needs Developer
/// Mode or an administrator.
pub fn symlink(target: impl AsRef<Path>, link: impl AsRef<Path>) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let target = target.as_ref();
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link).unwrap();
        } else {
            std::os::windows::fs::symlink_file(target, link).unwrap();
        }
    }
}

/// A folder for a test's Journals, on the same disk as `scan_root`'s folders
/// so its `FakeTrash` can rename items into it.
pub fn journal_dir() -> TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    tempfile::Builder::new()
        .prefix("journal-")
        .tempdir_in(base)
        .unwrap()
}

/// An OS Trash that moves items into a folder beside the Journals, so tests
/// never touch a real Trash. It can refuse items, or say up front that
/// everything in a folder can't be trashed.
pub struct FakeTrash {
    folder: PathBuf,
    pub refuses: Vec<PathBuf>,
    pub blocks: Vec<PathBuf>,
}

impl FakeTrash {
    pub fn new(journal: &Path) -> Self {
        FakeTrash {
            folder: journal.join("fake-trash"),
            refuses: Vec::new(),
            blocks: Vec::new(),
        }
    }
}

impl OsTrash for FakeTrash {
    fn blocked(&self, folder: &Path) -> Option<String> {
        self.blocks
            .iter()
            .any(|blocked| blocked == folder)
            .then(|| "This drive has no Trash".to_string())
    }

    fn trash(&self, path: &Path) -> Result<PathBuf, String> {
        if self.refuses.iter().any(|refused| refused == path) {
            return Err("The fake Trash refused it".to_string());
        }
        fs::create_dir_all(&self.folder).unwrap();
        let name = path.file_name().unwrap().to_string_lossy();
        let landed = (1..)
            .map(|n| self.folder.join(format!("{n} {name}")))
            .find(|landed| !landed.exists())
            .unwrap();
        fs::rename(path, &landed).map_err(|e| e.to_string())?;
        Ok(landed)
    }

    fn put_back(&self, landed: &Path, to: &Path) -> Result<(), String> {
        if to.exists() {
            return Err(format!("{} already exists", to.display()));
        }
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::rename(landed, to).map_err(|e| e.to_string())
    }
}

/// An OS Trash for building Plans, which has nothing to refuse.
fn no_trash() -> FakeTrash {
    FakeTrash::new(Path::new(""))
}

pub fn run(plan: &Plan, approved: &[String], journal: &Path) -> Result<Summary, SessionError> {
    core::run(plan, approved, journal, &FakeTrash::new(journal))
}

pub fn run_with_progress(
    plan: &Plan,
    approved: &[String],
    journal: &Path,
    on_progress: impl FnMut(RunProgress),
) -> Result<Summary, SessionError> {
    core::run_with_progress(
        plan,
        approved,
        journal,
        &FakeTrash::new(journal),
        on_progress,
    )
}

pub fn undo(session_id: &str, journal: &Path) -> Result<Summary, SessionError> {
    core::undo(session_id, journal, &FakeTrash::new(journal))
}

pub fn undo_operations(
    session_id: &str,
    operation_ids: &[String],
    journal: &Path,
) -> Result<Summary, SessionError> {
    core::undo_operations(session_id, operation_ids, journal, &FakeTrash::new(journal))
}

pub fn redo_operations(
    session_id: &str,
    operation_ids: &[String],
    journal: &Path,
) -> Result<Summary, SessionError> {
    core::redo_operations(session_id, operation_ids, journal, &FakeTrash::new(journal))
}

pub fn retry_operations(
    session_id: &str,
    operation_ids: &[String],
    journal: &Path,
) -> Result<Summary, SessionError> {
    core::retry_operations(session_id, operation_ids, journal, &FakeTrash::new(journal))
}

pub fn build_plan(scan: &ScanResult, config: &Config) -> Plan {
    core::build_plan(scan, config, &no_trash())
}

pub fn build_plan_with_progress(
    scan: &ScanResult,
    config: &Config,
    on_progress: impl FnMut(DuplicateProgress),
) -> Plan {
    core::build_plan_with_progress(scan, config, &no_trash(), on_progress)
}

/// Scans `root` and plans the Suggested Folder `name`, sending it where
/// `choice` says with the files at `files`, relative to `root`.
pub fn folder_plan(root: &Path, name: &str, choice: FolderChoice, files: &[&str]) -> Plan {
    let scanned = scan(root, &empty_config(root)).unwrap();
    let folder = SuggestedFolderChoice {
        name: name.to_string(),
        choice,
        files: files
            .iter()
            .map(|file| (real_path(&root.join(file)), FileChoice::InFolder))
            .collect(),
    };
    build_suggested_folders_plan(&scanned, &[folder]).unwrap()
}

pub fn build_review_plan(
    scan: &ScanResult,
    duplicates: &[DuplicateChoice],
    folders: &[SuggestedFolderChoice],
) -> Result<Plan, SuggestedFolderError> {
    core::build_review_plan(scan, &no_trash(), duplicates, folders)
}

pub fn build_suggested_folders_plan(
    scan: &ScanResult,
    folders: &[SuggestedFolderChoice],
) -> Result<Plan, SuggestedFolderError> {
    core::build_suggested_folders_plan(scan, &no_trash(), folders)
}
