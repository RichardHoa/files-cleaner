//! Sizes, then partial hashes, then full hashes, so most files are never read
//! in full.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::Hash;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{self, AtomicBool};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::config::DefaultCopyRule;
use crate::scanner::{ScanEntry, ScanResult};

/// Bytes read from each end of a file for its partial hash.
const PARTIAL_BYTES: u64 = 64 * 1024;
/// Bytes read at a time for a full hash, so progress moves within a large file.
const FULL_HASH_CHUNK: usize = 1024 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicatePhase {
    Comparing,
    Checking,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProgress {
    pub phase: DuplicatePhase,
    pub finished: u64,
    pub total: u64,
    pub current: PathBuf,
}

pub struct Found<'a> {
    pub kept: &'a ScanEntry,
    pub copies: Vec<&'a ScanEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub size: u64,
    pub copies: Vec<DuplicateCopy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCopy {
    pub path: PathBuf,
    pub relative_path: Vec<String>,
    /// Milliseconds since the Unix epoch.
    pub modified: i64,
    pub created: Option<i64>,
}

pub fn duplicate_groups(
    scan: &ScanResult,
    rule: DefaultCopyRule,
    cancel: &AtomicBool,
    on_progress: impl FnMut(DuplicateProgress),
) -> Option<Vec<DuplicateGroup>> {
    let copy = |entry: &ScanEntry| DuplicateCopy {
        path: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        modified: entry.modified,
        created: entry.created,
    };
    let found = find(&scan.entries, rule, cancel, on_progress)?;
    Some(
        found
            .into_iter()
            .map(|found| DuplicateGroup {
                size: found.kept.size,
                copies: std::iter::once(found.kept)
                    .chain(found.copies)
                    .map(copy)
                    .collect(),
            })
            .collect(),
    )
}

/// Leaves out 0-byte files, which are identical without being copies, and
/// Packages.
pub fn find<'a>(
    entries: &'a [ScanEntry],
    rule: DefaultCopyRule,
    cancel: &AtomicBool,
    on_progress: impl FnMut(DuplicateProgress),
) -> Option<Vec<Found<'a>>> {
    let cancelled = || cancel.load(atomic::Ordering::Relaxed);
    let candidates: Vec<&ScanEntry> = entries
        .iter()
        .filter(|e| !e.is_package && e.size > 0)
        .collect();
    let mut progress = Progress::new(on_progress);

    let same_size: Vec<Vec<&ScanEntry>> = split(candidates, |e| Some(e.size))
        .into_iter()
        .map(one_name_per_file)
        .filter(|group| group.len() > 1)
        .collect();
    progress.start(
        DuplicatePhase::Comparing,
        same_size.iter().map(|group| group.len() as u64).sum(),
    );
    let same_start_and_end: Vec<Vec<&ScanEntry>> = same_size
        .into_iter()
        .flat_map(|group| {
            split(group, |e| {
                if cancelled() {
                    return None;
                }
                progress.advance(&e.path, 1);
                partial_hash(&e.path, e.size).ok()
            })
        })
        .collect();
    if cancelled() {
        return None;
    }

    progress.start(
        DuplicatePhase::Checking,
        same_start_and_end.iter().flatten().map(|e| e.size).sum(),
    );
    let mut groups = Vec::new();
    for group in same_start_and_end {
        for mut same in split(group, |e| {
            if cancelled() {
                return None;
            }
            full_hash(&e.path, |read| progress.advance(&e.path, read)).ok()
        }) {
            // Comparing dates only some copies have would make the order
            // inconsistent.
            let by_created = same.iter().all(|e| e.created.is_some());
            same.sort_by(|a, b| copy_order(a, b, rule, by_created));
            let kept = same.remove(0);
            groups.push(Found { kept, copies: same });
        }
    }
    if cancelled() {
        return None;
    }
    groups.sort_by(|a, b| a.kept.path.cmp(&b.kept.path));
    Some(groups)
}

struct Progress<F: FnMut(DuplicateProgress)> {
    phase: DuplicatePhase,
    finished: u64,
    total: u64,
    last_report: Option<Instant>,
    report: F,
}

impl<F: FnMut(DuplicateProgress)> Progress<F> {
    fn new(report: F) -> Self {
        Progress {
            phase: DuplicatePhase::Comparing,
            finished: 0,
            total: 0,
            last_report: None,
            report,
        }
    }

    fn start(&mut self, phase: DuplicatePhase, total: u64) {
        self.phase = phase;
        self.finished = 0;
        self.total = total;
        self.last_report = None;
    }

    fn advance(&mut self, current: &Path, amount: u64) {
        if self
            .last_report
            .is_none_or(|last| last.elapsed() >= PROGRESS_INTERVAL)
        {
            (self.report)(DuplicateProgress {
                phase: self.phase,
                finished: self.finished,
                total: self.total,
                current: current.to_path_buf(),
            });
            self.last_report = Some(Instant::now());
        }
        self.finished += amount;
    }
}

fn split<K: Hash + Eq>(
    entries: Vec<&ScanEntry>,
    mut key: impl FnMut(&ScanEntry) -> Option<K>,
) -> Vec<Vec<&ScanEntry>> {
    if entries.len() < 2 {
        return Vec::new();
    }
    let mut by_key: HashMap<K, Vec<&ScanEntry>> = HashMap::new();
    for entry in entries {
        if let Some(k) = key(entry) {
            by_key.entry(k).or_default().push(entry);
        }
    }
    by_key
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

/// Hard links are one file: trashing one name frees nothing.
fn one_name_per_file(mut entries: Vec<&ScanEntry>) -> Vec<&ScanEntry> {
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let mut seen = HashSet::new();
    entries.retain(|e| file_id(&e.path).is_none_or(|id| seen.insert(id)));
    entries
}

/// Windows can't tell in stable Rust, so there each name counts as its own
/// file.
#[cfg(unix)]
fn file_id(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(path).ok()?;
    (metadata.nlink() > 1).then(|| (metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn file_id(_path: &Path) -> Option<(u64, u64)> {
    None
}

/// Always ends in a tie-break, so the kept copy never depends on scan order.
fn copy_order(a: &ScanEntry, b: &ScanEntry, rule: DefaultCopyRule, by_created: bool) -> Ordering {
    let created = if by_created {
        b.created.cmp(&a.created)
    } else {
        Ordering::Equal
    };
    let newest = b.modified.cmp(&a.modified).then(created);
    let shortest = a.path.as_os_str().len().cmp(&b.path.as_os_str().len());
    match rule {
        DefaultCopyRule::Newest => newest.then(shortest),
        DefaultCopyRule::Oldest => newest.reverse().then(shortest),
        DefaultCopyRule::ShortestPath => shortest.then(newest),
    }
    .then(a.path.cmp(&b.path))
}

fn partial_hash(path: &Path, size: u64) -> io::Result<[u8; 32]> {
    // A file in the Scan Root the user chose; no path comes from a web request.
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0; PARTIAL_BYTES.min(size) as usize];
    file.read_exact(&mut buffer)?;
    hasher.update(&buffer);
    if size > PARTIAL_BYTES {
        file.seek(SeekFrom::Start(
            size.saturating_sub(PARTIAL_BYTES).max(PARTIAL_BYTES),
        ))?;
        buffer.clear();
        file.read_to_end(&mut buffer)?;
        hasher.update(&buffer);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn full_hash(path: &Path, mut on_read: impl FnMut(u64)) -> io::Result<[u8; 32]> {
    // A file in the Scan Root the user chose; no path comes from a web request.
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0; FULL_HASH_CHUNK];
    loop {
        let read = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        hasher.update(&buffer[..read]);
        on_read(read as u64);
    }
    Ok(*hasher.finalize().as_bytes())
}
