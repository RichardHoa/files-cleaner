use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::Config;
use crate::protection::{is_package, Protection};

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
    /// Files and folders that couldn't be read, so weren't scanned.
    pub unreadable: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum ScanError {
    /// The Scan Root is, or is inside, a Protected Path or a Package.
    ProtectedRoot { root: PathBuf, reason: String },
    Io(io::Error),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScanError::ProtectedRoot { reason, .. } => f.write_str(reason),
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

/// Walks `scan_root` recursively and lists every file and Package under it,
/// skipping hidden entries, symlinks and Protected Paths.
pub fn scan(scan_root: &Path, config: &Config) -> Result<ScanResult, ScanError> {
    let protection = Protection::new(config);
    if let Some(reason) = protection.refusal(scan_root) {
        return Err(ScanError::ProtectedRoot { root: scan_root.to_path_buf(), reason });
    }
    let root = fs::canonicalize(scan_root)?;
    let mut result = ScanResult {
        entries: Vec::new(),
        skipped: 0,
        unreadable: Vec::new(),
        root: root.clone(),
    };
    // The Scan Root itself must be readable; anything below it that isn't
    // is reported and passed over.
    let mut folders = Vec::new();
    let mut next = Some(fs::read_dir(&root)?);
    while let Some(children) = next.take().or_else(|| open_next(&mut folders, &mut result.unreadable)) {
        for child in children {
            let Ok(child) = child else { continue };
            let path = child.path();
            if protection.reason(&path).is_some() {
                result.skipped += 1;
                continue;
            }
            if child.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            match visit(path.clone(), &mut folders) {
                Ok(Some(entry)) => result.entries.push(entry),
                Ok(None) => {}
                Err(_) => result.unreadable.push(path),
            }
        }
    }
    result.entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

/// Opens the next queued folder, recording any that can't be read.
fn open_next(folders: &mut Vec<PathBuf>, unreadable: &mut Vec<PathBuf>) -> Option<fs::ReadDir> {
    while let Some(folder) = folders.pop() {
        match fs::read_dir(&folder) {
            Ok(children) => return Some(children),
            Err(_) => unreadable.push(folder),
        }
    }
    None
}

/// Lists a file or Package, or queues a folder to walk into.
fn visit(path: PathBuf, folders: &mut Vec<PathBuf>) -> io::Result<Option<ScanEntry>> {
    // Never follows symlinks. Finder aliases are plain files, so they are
    // listed but never resolved either.
    let metadata = fs::symlink_metadata(&path)?;
    let kind = metadata.file_type();
    if kind.is_dir() && is_package(&path) {
        let size = total_size(&path);
        Ok(Some(entry(path, &metadata, size, true)?))
    } else if kind.is_dir() {
        folders.push(path);
        Ok(None)
    } else if kind.is_file() {
        Ok(Some(entry(path, &metadata, metadata.len(), false)?))
    } else {
        Ok(None)
    }
}

fn entry(path: PathBuf, metadata: &fs::Metadata, size: u64, is_package: bool) -> io::Result<ScanEntry> {
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let extension = path.extension().unwrap_or_default().to_string_lossy().into_owned();
    Ok(ScanEntry {
        name,
        extension,
        size,
        modified: millis(metadata.modified()?),
        created: metadata.created().ok().map(millis),
        path,
        is_package,
    })
}

/// Total bytes of every file inside `folder`, hidden ones included, without
/// following symlinks. Anything inside that can't be read is left out, so
/// the Package is still listed.
fn total_size(folder: &Path) -> u64 {
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

fn millis(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_millis() as i64,
        Err(before) => -(before.duration().as_millis() as i64),
    }
}
