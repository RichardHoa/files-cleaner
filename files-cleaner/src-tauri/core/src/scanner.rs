use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::Config;
use crate::protection::{canonical, is_hidden, is_package, Protection};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanEntry {
    pub name: String,
    /// Without the dot, as written on disk.
    pub extension: String,
    /// Bytes; for a Package, everything inside it.
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub modified: i64,
    /// `None` where the file system doesn't record it.
    pub created: Option<i64>,
    pub path: PathBuf,
    pub relative_path: Vec<String>,
    pub is_package: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub entries: Vec<ScanEntry>,
    pub root: PathBuf,
    /// Protected Paths met and left alone.
    pub skipped: usize,
    /// Checked for duplicates but never sorted into Suggested Folders.
    pub no_extension: usize,
    pub unreadable: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub found: usize,
    pub folders: usize,
    pub skipped: usize,
    /// The folder being read.
    pub current: PathBuf,
}

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub enum ScanError {
    Io(io::Error),
    Cancelled,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScanError::Io(e) => write!(f, "Couldn't read the folder: {e}"),
            ScanError::Cancelled => write!(f, "The scan was cancelled"),
        }
    }
}

impl std::error::Error for ScanError {}

impl From<io::Error> for ScanError {
    fn from(e: io::Error) -> Self {
        ScanError::Io(e)
    }
}

/// A warning only: any folder can still be scanned.
pub fn scan_root_warning(scan_root: &Path, config: &Config) -> Option<String> {
    Protection::new(config).refusal(scan_root)
}

/// Protected Paths covering `scan_root` itself are waived: the user accepted
/// `scan_root_warning`.
pub fn scan(scan_root: &Path, config: &Config) -> Result<ScanResult, ScanError> {
    scan_with_progress(scan_root, config, &AtomicBool::new(false), |_| {})
}

pub fn scan_with_progress(
    scan_root: &Path,
    config: &Config,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(ScanProgress),
) -> Result<ScanResult, ScanError> {
    let root = canonical(scan_root)?;
    let protection = Protection::new(config);
    let mut result = ScanResult {
        entries: Vec::new(),
        skipped: 0,
        no_extension: 0,
        unreadable: Vec::new(),
        root: root.clone(),
    };
    let mut folders_walked = 0;
    let mut last_report = Instant::now();
    let mut report = |result: &ScanResult, folders_walked: usize, current: &Path| {
        on_progress(ScanProgress {
            found: result.entries.len(),
            folders: folders_walked,
            skipped: result.skipped,
            current: current.to_path_buf(),
        })
    };
    let mut folders = vec![root.clone()];
    while let Some(folder) = folders.pop() {
        // An unreadable Scan Root is an error; anything below it is only passed
        // over.
        let children = match fs::read_dir(&folder) {
            Ok(children) => children,
            Err(e) if folder == root => return Err(e.into()),
            Err(_) => {
                result.unreadable.push(folder);
                continue;
            }
        };
        folders_walked += 1;
        for child in children {
            if cancel.load(Ordering::Relaxed) {
                return Err(ScanError::Cancelled);
            }
            if last_report.elapsed() >= PROGRESS_INTERVAL {
                report(&result, folders_walked, &folder);
                last_report = Instant::now();
            }
            let Ok(child) = child else { continue };
            let path = child.path();
            // The link's own type, so symlinks are never followed.
            let Ok(kind) = child.file_type() else {
                result.unreadable.push(path);
                continue;
            };
            if kind.is_symlink() || !(kind.is_dir() || kind.is_file()) {
                continue;
            }
            if protection.reason(&path, kind.is_dir()).is_some() {
                result.skipped += 1;
                continue;
            }
            if is_hidden(&child) {
                continue;
            }
            if kind.is_file() && path.extension().is_none_or(|e| e.is_empty()) {
                result.no_extension += 1;
            }
            if kind.is_dir() && !is_package(&path) {
                folders.push(path);
                continue;
            }
            match child
                .metadata()
                .and_then(|metadata| entry(&path, &root, &metadata, kind.is_dir()))
            {
                Ok(entry) => result.entries.push(entry),
                Err(_) => result.unreadable.push(path),
            }
        }
    }
    report(&result, folders_walked, &root);
    result.entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

fn entry(
    path: &Path,
    root: &Path,
    metadata: &fs::Metadata,
    is_package: bool,
) -> io::Result<ScanEntry> {
    let relative = path
        .strip_prefix(root)
        .expect("the scanner only walks inside the Scan Root");
    Ok(ScanEntry {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        extension: path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        size: if is_package {
            total_size(path)
        } else {
            metadata.len()
        },
        modified: millis(metadata.modified()?),
        created: metadata.created().ok().map(millis),
        path: path.to_path_buf(),
        relative_path: relative
            .iter()
            .map(|name| name.to_string_lossy().into_owned())
            .collect(),
        is_package,
    })
}

/// Hidden files count and unreadable ones don't, so the Package is always
/// listed.
pub(crate) fn total_size(folder: &Path) -> u64 {
    let mut total = 0;
    let mut folders = vec![folder.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(children) = fs::read_dir(&folder) else {
            continue;
        };
        for child in children.flatten() {
            let Ok(metadata) = child.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                folders.push(child.path());
            } else if metadata.is_file() {
                total += metadata.len();
            }
        }
    }
    total
}

/// `Path::exists` would say no for a broken symlink.
pub(crate) fn exists(path: &Path) -> bool {
    !matches!(fs::symlink_metadata(path), Err(e) if e.kind() == io::ErrorKind::NotFound)
}

pub(crate) fn millis(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_millis() as i64,
        Err(before) => -(before.duration().as_millis() as i64),
    }
}
