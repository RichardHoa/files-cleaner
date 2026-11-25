//! Names that differ only in letter case count as taken on every OS, since
//! disks move between machines.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// An unreadable folder has no names taken: it may not exist yet.
pub fn names_in(folder: &Path) -> HashSet<String> {
    let Ok(children) = fs::read_dir(folder) else {
        return HashSet::new();
    };
    children
        .flatten()
        .map(|child| child.file_name().to_string_lossy().to_lowercase())
        .collect()
}

/// Counts on from a suffix already there: `notes (1).pptx` becomes `notes
/// (2).pptx`.
pub fn free_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(&name.to_lowercase()) {
        return name.to_string();
    }
    let (stem, extension) = split_extension(name);
    let (base, first) = match strip_suffix(stem) {
        Some((base, n)) => (base, n + 1),
        None => (stem, 1),
    };
    (first..)
        .map(|n| format!("{base} ({n}){extension}"))
        .find(|candidate| !taken.contains(&candidate.to_lowercase()))
        .expect("there is always a free suffix")
}

/// A folder has no extension, so `v1.2` becomes `v1.2 (1)`, not `v1 (1).2`.
pub fn free_folder_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(&name.to_lowercase()) {
        return name.to_string();
    }
    (1..)
        .map(|n| format!("{name} ({n})"))
        .find(|candidate| !taken.contains(&candidate.to_lowercase()))
        .expect("there is always a free suffix")
}

/// A leading dot marks a hidden file, so it doesn't start an extension.
pub fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    }
}

fn strip_suffix(stem: &str) -> Option<(&str, u64)> {
    let inner = stem.strip_suffix(')')?;
    let open = inner.rfind(" (")?;
    let digits = &inner[open + 2..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((&inner[..open], digits.parse().ok()?))
}
