#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use files_cleaner_core::{
    self as core, load_config, scan, Config, DuplicateChoice, DuplicateProgress, FileChoice,
    FolderChoice, OsTrash, Plan, RunProgress, ScanResult, SessionError, SuggestedFolderChoice,
    SuggestedFolderError, Summary,
};
use tempfile::TempDir;

/// Not the system temp folder or `target/`: both are Protected Paths, which
/// would hide the test's files.
pub fn scan_root() -> TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(base)
        .unwrap()
}

/// `fs::canonicalize` returns `\\?\C:\…` on Windows, which the app never shows.
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

/// On Windows this needs Developer Mode or an administrator.
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

/// On the same disk as `scan_root`'s folders, so `FakeTrash` can rename items
/// into it.
pub fn journal_dir() -> TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    tempfile::Builder::new()
        .prefix("journal-")
        .tempdir_in(base)
        .unwrap()
}

pub fn journal_database(journal: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(journal.join("journal.sqlite")).unwrap()
}

/// As a crash right after its changes would leave it.
pub fn drop_outcomes(journal: &Path, id: &str) {
    journal_database(journal)
        .execute(
            "DELETE FROM records WHERE session_id = ?1
             AND json_extract(record, '$.record') IN ('outcome', 'finished')",
            [id],
        )
        .unwrap();
}

/// So tests never touch a real Trash.
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

/// `None` if that isn't allowed: it needs root, or `sudo` without a password,
/// as on CI.
#[cfg(target_os = "linux")]
pub struct OtherDisk(PathBuf);

#[cfg(target_os = "linux")]
impl OtherDisk {
    pub fn mount(folder: &Path) -> Option<OtherDisk> {
        fs::create_dir_all(folder).unwrap();
        let mut args = vec!["mount", "-t", "tmpfs", "-o", "size=1m,mode=1777", "tmpfs"];
        args.push(folder.to_str().unwrap());
        as_root(&args).then(|| OtherDisk(folder.to_path_buf()))
    }

    pub fn folder(&self) -> &Path {
        &self.0
    }
}

#[cfg(target_os = "linux")]
impl Drop for OtherDisk {
    fn drop(&mut self) {
        as_root(&["umount", self.0.to_str().unwrap()]);
    }
}

#[cfg(target_os = "linux")]
fn as_root(args: &[&str]) -> bool {
    let succeeds = |program: &str, args: &[&str]| {
        std::process::Command::new(program)
            .args(args)
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    succeeds(args[0], &args[1..]) || succeeds("sudo", &[&["-n"], args].concat())
}
