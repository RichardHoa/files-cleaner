//! Finds Duplicate Groups: files with byte-identical contents, whatever
//! their names. Compares sizes first, then a partial hash of each file's
//! start and end, then a full BLAKE3 hash, so most files are never read.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::scanner::{ScanEntry, ScanResult};

/// Bytes read from each end of a file for its partial hash.
const PARTIAL_BYTES: u64 = 64 * 1024;
/// Bytes read at a time for a full hash, so progress moves within a large file.
const FULL_HASH_CHUNK: usize = 1024 * 1024;
/// The longest finding duplicates waits between progress reports.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// What finding duplicates is doing.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicatePhase {
    /// Comparing the start and end of files of the same size; counted in files.
    Comparing,
    /// Reading in full the files whose start and end match; counted in bytes.
    Checking,
}

/// How far finding duplicates has got, reported while it runs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProgress {
    pub phase: DuplicatePhase,
    /// Files compared, or bytes checked, so far in this phase.
    pub finished: u64,
    /// Files to compare, or bytes to check, in this phase.
    pub total: u64,
    /// The file being read.
    pub current: PathBuf,
}

/// Two or more files with the same contents, the Keeper first.
pub struct Found<'a> {
    pub keeper: &'a ScanEntry,
    pub copies: Vec<&'a ScanEntry>,
}

/// A Duplicate Group as the review screen shows it: every copy, the default
/// Keeper first.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// Bytes of one copy.
    pub size: u64,
    pub copies: Vec<DuplicateCopy>,
}

/// One copy in a Duplicate Group.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCopy {
    pub path: PathBuf,
    /// The names from the Scan Root down to the copy, its own name last.
    pub relative_path: Vec<String>,
    /// Milliseconds since the Unix epoch.
    pub modified: i64,
    pub created: Option<i64>,
}

/// Every Duplicate Group in `scan`, as `find` finds them.
pub fn duplicate_groups(
    scan: &ScanResult,
    on_progress: impl FnMut(DuplicateProgress),
) -> Vec<DuplicateGroup> {
    let copy = |entry: &ScanEntry| DuplicateCopy {
        path: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        modified: entry.modified,
        created: entry.created,
    };
    find(&scan.entries, on_progress)
        .into_iter()
        .map(|found| DuplicateGroup {
            size: found.keeper.size,
            copies: std::iter::once(found.keeper)
                .chain(found.copies)
                .map(copy)
                .collect(),
        })
        .collect()
}

/// Every Duplicate Group among `entries`, leaving out 0-byte files and
/// Packages. Files that can't be read are left out too. Calls `on_progress`
/// at most every `PROGRESS_INTERVAL` in each `DuplicatePhase`: first while
/// comparing the start and end of every file that shares its size, then
/// while reading in full those that still match.
pub fn find(entries: &[ScanEntry], on_progress: impl FnMut(DuplicateProgress)) -> Vec<Found<'_>> {
    let candidates: Vec<&ScanEntry> = entries
        .iter()
        .filter(|e| !e.is_package && e.size > 0)
        .collect();
    let mut progress = Progress::new(on_progress);

    let same_size = split(candidates, |e| Some(e.size));
    progress.start(
        DuplicatePhase::Comparing,
        same_size.iter().map(|group| group.len() as u64).sum(),
    );
    let same_start_and_end: Vec<Vec<&ScanEntry>> = same_size
        .into_iter()
        .flat_map(|group| {
            split(group, |e| {
                progress.advance(&e.path, 1);
                partial_hash(&e.path, e.size).ok()
            })
        })
        .collect();

    progress.start(
        DuplicatePhase::Checking,
        same_start_and_end.iter().flatten().map(|e| e.size).sum(),
    );
    let mut groups = Vec::new();
    for group in same_start_and_end {
        for mut same in split(group, |e| {
            full_hash(&e.path, |read| progress.advance(&e.path, read)).ok()
        }) {
            // Created dates are compared only if every copy has one, so the
            // order stays consistent whichever copies are compared.
            let by_created = same.iter().all(|e| e.created.is_some());
            same.sort_by(|a, b| keeper_order(a, b, by_created));
            let keeper = same.remove(0);
            groups.push(Found {
                keeper,
                copies: same,
            });
        }
    }
    groups.sort_by(|a, b| a.keeper.path.cmp(&b.keeper.path));
    groups
}

/// Reports progress through one phase at a time, at most every `PROGRESS_INTERVAL`.
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

    /// Starts `phase`, with `total` files or bytes to go through.
    fn start(&mut self, phase: DuplicatePhase, total: u64) {
        self.phase = phase;
        self.finished = 0;
        self.total = total;
        self.last_report = None;
    }

    /// Called as `amount` more files or bytes of `current` are read.
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

/// Splits `entries` by `key`, keeping only groups of two or more. Entries
/// whose key is `None` are dropped.
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

/// Which of two copies is the better Keeper (sorts first): the newest by
/// modified date, then by created date if `by_created`, then the shortest
/// path, then the first alphabetically.
fn keeper_order(a: &ScanEntry, b: &ScanEntry, by_created: bool) -> Ordering {
    let created = if by_created {
        b.created.cmp(&a.created)
    } else {
        Ordering::Equal
    };
    b.modified
        .cmp(&a.modified)
        .then(created)
        .then(a.path.as_os_str().len().cmp(&b.path.as_os_str().len()))
        .then(a.path.cmp(&b.path))
}

/// A hash of the file's first and last `PARTIAL_BYTES`.
fn partial_hash(path: &Path, size: u64) -> io::Result<[u8; 32]> {
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

/// A hash of the whole file, calling `on_read` with the bytes of each chunk read.
fn full_hash(path: &Path, mut on_read: impl FnMut(u64)) -> io::Result<[u8; 32]> {
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
