use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::Config;
use crate::protection::{canonical, is_hidden, is_package, Protection};

/// One scanned file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanEntry {
    pub name: String,
    /// The extension as written on disk, without the dot; empty if none.
    pub extension: String,
    /// Bytes; for a Package, the total of everything inside it.
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub modified: i64,
    /// Milliseconds since the Unix epoch, if the file system records it.
    pub created: Option<i64>,
    pub path: PathBuf,
    /// The names from the Scan Root down to this item, its own name last.
    pub relative_path: Vec<String>,
    pub is_package: bool,
}

/// Everything a scan found under a Scan Root.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub entries: Vec<ScanEntry>,
    /// The Scan Root, with symlinks resolved.
    pub root: PathBuf,
    /// How many Protected Paths were met and left alone.
    pub skipped: usize,
    /// How many files were left out because they have no extension.
    pub no_extension: usize,
    /// Files and folders that couldn't be read, so weren't scanned.
    pub unreadable: Vec<PathBuf>,
}

/// How far a scan has got, reported while it runs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    /// Files and Packages listed so far.
    pub found: usize,
    /// Folders walked into so far, the Scan Root included.
    pub folders: usize,
    /// Protected Paths left alone so far.
    pub skipped: usize,
    /// The folder being read.
    pub current: PathBuf,
}

/// The longest a scan waits between progress reports.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub enum ScanError {
    Io(io::Error),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScanError::Io(e) => write!(f, "Couldn't read the folder: {e}"),
        }
    }
}

impl std::error::Error for ScanError {}

impl From<io::Error> for ScanError {
    fn from(e: io::Error) -> Self {
        ScanError::Io(e)
    }
}

/// Why the user should think twice before scanning `scan_root`, if they
/// should: it is, or is inside, a Protected Path or a Package. Any folder can
/// still be scanned.
pub fn scan_root_warning(scan_root: &Path, config: &Config) -> Option<String> {
    Protection::new(config).refusal(scan_root)
}

/// Walks `scan_root` recursively and lists every file and Package under it,
/// skipping hidden entries, symlinks, Protected Paths inside it and files with
/// no extension. Protected Paths covering `scan_root` itself are waived: see
/// `scan_root_warning`.
pub fn scan(scan_root: &Path, config: &Config) -> Result<ScanResult, ScanError> {
    scan_with_progress(scan_root, config, |_| {})
}

/// Like `scan`, calling `on_progress` every `PROGRESS_INTERVAL` or so while it
/// runs, and once more at the end with the final counts.
pub fn scan_with_progress(
    scan_root: &Path,
    config: &Config,
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
        // The Scan Root itself must be readable; anything below it that
        // isn't is reported and passed over.
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
            if last_report.elapsed() >= PROGRESS_INTERVAL {
                report(&result, folders_walked, &folder);
                last_report = Instant::now();
            }
            let Ok(child) = child else { continue };
            let path = child.path();
            // Never follows symlinks: the type is the link's own. macOS
            // aliases and Windows shortcuts are plain files, so they are
            // listed but never resolved.
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
                continue;
            }
            if kind.is_dir() && !is_package(&path) {
                folders.push(path);
                continue;
            }
            // What's left is a file, or a folder that is a Package.
            match child.metadata().and_then(|metadata| entry(&path, &root, &metadata, kind.is_dir())) {
                Ok(entry) => result.entries.push(entry),
                Err(_) => result.unreadable.push(path),
            }
        }
    }
    report(&result, folders_walked, &root);
    result.entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

/// Lists a file, or a Package with the total size of what's inside it.
fn entry(path: &Path, root: &Path, metadata: &fs::Metadata, is_package: bool) -> io::Result<ScanEntry> {
    let relative = path.strip_prefix(root).expect("the scanner only walks inside the Scan Root");
    Ok(ScanEntry {
        name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        extension: path.extension().unwrap_or_default().to_string_lossy().into_owned(),
        size: if is_package { total_size(path) } else { metadata.len() },
        modified: millis(metadata.modified()?),
        created: metadata.created().ok().map(millis),
        path: path.to_path_buf(),
        relative_path: relative.iter().map(|name| name.to_string_lossy().into_owned()).collect(),
        is_package,
    })
}

/// Total bytes of every file inside `folder`, hidden ones included, without
/// following symlinks. Anything inside that can't be read is left out, so
/// the Package is still listed.
pub(crate) fn total_size(folder: &Path) -> u64 {
    let mut total = 0;
    let mut folders = vec![folder.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(children) = fs::read_dir(&folder) else { continue };
        for child in children.flatten() {
            let Ok(metadata) = child.metadata() else { continue };
            if metadata.is_dir() {
                folders.push(child.path());
            } else if metadata.is_file() {
                total += metadata.len();
            }
        }
    }
    total
}

pub(crate) fn millis(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_millis() as i64,
        Err(before) => -(before.duration().as_millis() as i64),
    }
}
