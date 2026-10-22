//! Finds Duplicate Groups: files with byte-identical contents, whatever
//! their names. Compares sizes first, then a partial hash of each file's
//! start and end, then a full BLAKE3 hash, so most files are never read.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::scanner::ScanEntry;

/// Bytes read from each end of a file for its partial hash.
const PARTIAL_BYTES: u64 = 64 * 1024;

/// Two or more files with the same contents, the Keeper first.
pub struct DuplicateGroup<'a> {
    pub keeper: &'a ScanEntry,
    pub copies: Vec<&'a ScanEntry>,
}

/// Every Duplicate Group among `entries`, leaving out 0-byte files and
/// Packages. Files that can't be read are left out too.
pub fn find(entries: &[ScanEntry]) -> Vec<DuplicateGroup<'_>> {
    let candidates: Vec<&ScanEntry> = entries
        .iter()
        .filter(|e| !e.is_package && e.size > 0)
        .collect();
    let mut groups = Vec::new();
    for same_size in split(candidates, |e| Some(e.size)) {
        for same_start_and_end in split(same_size, |e| partial_hash(&e.path, e.size).ok()) {
            for mut same in split(same_start_and_end, |e| full_hash(&e.path).ok()) {
                // Created dates are compared only if every copy has one, so the
                // order stays consistent whichever copies are compared.
                let by_created = same.iter().all(|e| e.created.is_some());
                same.sort_by(|a, b| keeper_order(a, b, by_created));
                let keeper = same.remove(0);
                groups.push(DuplicateGroup {
                    keeper,
                    copies: same,
                });
            }
        }
    }
    groups.sort_by(|a, b| a.keeper.path.cmp(&b.keeper.path));
    groups
}

/// Splits `entries` by `key`, keeping only groups of two or more. Entries
/// whose key is `None` are dropped.
fn split<K: Hash + Eq>(
    entries: Vec<&ScanEntry>,
    key: impl Fn(&ScanEntry) -> Option<K>,
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

fn full_hash(path: &Path) -> io::Result<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(path)?)?;
    Ok(*hasher.finalize().as_bytes())
}
