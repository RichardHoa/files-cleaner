use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::duplicates;
use crate::duplicates::DuplicateProgress;
use crate::os_trash::OsTrash;
use crate::protection::is_os_metadata_file;
use crate::scanner::{ScanEntry, ScanResult};
use crate::volume::different_disks;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub root: PathBuf,
    /// Ordered so folders exist before they're needed and are empty before
    /// they're trashed.
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    /// Stable across rebuilds, so ticks survive a new Plan.
    pub id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    pub relative_path: Vec<String>,
    pub destination: Vec<String>,
    /// `None` for Trash: only the OS knows where it lands, and only once it
    /// has.
    pub destination_path: Option<PathBuf>,
    /// What it was when scanned; if it has changed since, it is skipped.
    #[serde(flatten)]
    pub planned: SourceState,
    /// Why the Plan proposes it.
    pub reason: Reason,
    /// Trashing an emptied folder starts unticked.
    pub ticked: bool,
    /// An Operation with one can't be ticked and never runs.
    pub error: Option<String>,
}

/// Compared again before running, so a changed item is never touched. All zeros
/// for a folder to create.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceState {
    /// Bytes; for a Package, everything inside it.
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub modified: i64,
    pub is_package: bool,
    /// Its own date changes as it empties, so it is checked by its contents
    /// instead.
    #[serde(default)]
    pub is_folder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    Trash,
    Move,
    CreateFolder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Reason {
    /// `None` if no copy is kept.
    Duplicate {
        kept: Option<Vec<String>>,
    },
    EmptiedFolder,
    SuggestedFolder {
        folder: String,
    },
}

/// Anything that can't happen stays in the Plan as an error, so the user sees
/// why.
pub fn build_plan(scan: &ScanResult, config: &Config, os_trash: &dyn OsTrash) -> Plan {
    build_plan_with_progress(scan, config, os_trash, |_| {})
}

pub fn build_plan_with_progress(
    scan: &ScanResult,
    config: &Config,
    os_trash: &dyn OsTrash,
    on_progress: impl FnMut(DuplicateProgress),
) -> Plan {
    let root = &scan.root;
    let mut operations = Vec::new();
    let found = duplicates::find(
        &scan.entries,
        config.default_copy_rule,
        &AtomicBool::new(false),
        on_progress,
    )
    .unwrap_or_default();
    for group in found {
        for copy in group.copies {
            operations.push(trash(
                copy,
                Reason::Duplicate {
                    kept: Some(group.kept.relative_path.clone()),
                },
            ));
        }
    }
    finish(root, os_trash, operations, Vec::new())
}

pub(crate) fn finish(
    root: &Path,
    os_trash: &dyn OsTrash,
    mut operations: Vec<Operation>,
    mut created: Vec<(PathBuf, Reason)>,
) -> Plan {
    operations.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    // Sorted by path, each folder comes before the folders inside it.
    created.sort_by(|a, b| a.0.cmp(&b.0));
    let created = created
        .into_iter()
        .map(|(folder, reason)| create_folder_operation(root, folder, reason));
    let mut operations: Vec<Operation> = created.chain(operations).collect();
    // Asked per folder: the answer depends on the disk, and Plans are large.
    let mut blocked_folders: HashMap<PathBuf, Option<String>> = HashMap::new();
    let mut why_not = |op: &Operation| {
        across_disks(op).or_else(|| {
            let folder = op
                .source
                .parent()
                .filter(|_| op.kind == OperationKind::Trash)?;
            blocked_folders
                .entry(folder.to_path_buf())
                .or_insert_with(|| os_trash.blocked(folder))
                .clone()
        })
    };
    for op in &mut operations {
        op.error = why_not(op);
    }
    let emptied = emptied_folders(root, &operations);
    for folder in emptied {
        let mut op = trash_emptied_folder(root, &folder);
        op.error = why_not(&op);
        operations.push(op);
    }
    for op in &mut operations {
        op.ticked &= op.error.is_none();
    }
    Plan {
        root: root.to_path_buf(),
        operations,
    }
}

fn across_disks(op: &Operation) -> Option<String> {
    let destination = op
        .destination_path
        .as_deref()
        .filter(|_| op.kind == OperationKind::Move)?;
    if !different_disks(&op.source, destination) {
        return None;
    }
    let name = op
        .relative_path
        .last()
        .map_or_else(|| op.source.display().to_string(), Clone::clone);
    let to = destination.parent().unwrap_or(destination).display();
    Some(format!(
        "{name} is on a different disk from {to}, and the app only moves items within one disk"
    ))
}

/// Only the topmost of nested ones: trashing it takes the others along.
fn emptied_folders(root: &Path, operations: &[Operation]) -> Vec<PathBuf> {
    let runs = |op: &&Operation| op.error.is_none();
    let leaving: HashSet<&Path> = operations
        .iter()
        .filter(runs)
        .filter(|op| op.kind != OperationKind::CreateFolder)
        .map(|op| op.source.as_path())
        .collect();
    let arriving: Vec<&Path> = operations
        .iter()
        .filter(runs)
        .filter_map(|op| op.destination_path.as_deref())
        .collect();
    // Sorted by path, each folder comes before the folders inside it.
    let candidates: BTreeSet<&Path> = leaving
        .iter()
        .flat_map(|source| {
            source
                .ancestors()
                .skip(1)
                .take_while(|folder| folder.starts_with(root) && *folder != root)
        })
        .collect();
    let mut emptied: Vec<PathBuf> = Vec::new();
    let mut known = HashMap::new();
    for folder in candidates {
        if emptied.iter().any(|above| folder.starts_with(above)) {
            continue;
        }
        if empties(folder, &leaving, &arriving, &mut known) {
            emptied.push(folder.to_path_buf());
        }
    }
    emptied
}

fn empties(
    folder: &Path,
    leaving: &HashSet<&Path>,
    arriving: &[&Path],
    known: &mut HashMap<PathBuf, bool>,
) -> bool {
    if let Some(&answer) = known.get(folder) {
        return answer;
    }
    let answer = !arriving.iter().any(|to| to.starts_with(folder))
        && fs::read_dir(folder).is_ok_and(|children| {
            children.into_iter().all(|child| {
                let Ok(child) = child else { return false };
                let path = child.path();
                if leaving.contains(path.as_path()) {
                    return true;
                }
                match child.file_type() {
                    Ok(kind) if kind.is_file() => {
                        is_os_metadata_file(&child.file_name().to_string_lossy())
                    }
                    Ok(kind) if kind.is_dir() => empties(&path, leaving, arriving, known),
                    _ => false,
                }
            })
        });
    known.insert(folder.to_path_buf(), answer);
    answer
}

pub(crate) fn trash(entry: &ScanEntry, reason: Reason) -> Operation {
    Operation {
        id: format!("trash:{}", entry.relative_path.join("/")),
        kind: OperationKind::Trash,
        source: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        destination: Vec::new(),
        destination_path: None,
        planned: planned(entry),
        reason,
        ticked: true,
        error: None,
    }
}

fn trash_emptied_folder(root: &Path, folder: &Path) -> Operation {
    let relative_path = names_below(root, folder);
    Operation {
        id: format!("trashFolder:{}", relative_path.join("/")),
        kind: OperationKind::Trash,
        source: folder.to_path_buf(),
        relative_path,
        destination: Vec::new(),
        destination_path: None,
        planned: SourceState {
            size: 0,
            modified: 0,
            is_package: false,
            is_folder: true,
        },
        reason: Reason::EmptiedFolder,
        ticked: false,
        error: None,
    }
}

pub(crate) fn move_to(
    root: &Path,
    entry: &ScanEntry,
    destination: PathBuf,
    reason: Reason,
) -> Operation {
    Operation {
        id: format!("move:{}", entry.relative_path.join("/")),
        kind: OperationKind::Move,
        source: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        destination: names_below(root, &destination),
        destination_path: Some(destination),
        planned: planned(entry),
        reason,
        ticked: true,
        error: None,
    }
}

fn create_folder_operation(root: &Path, folder: PathBuf, reason: Reason) -> Operation {
    let names = names_below(root, &folder);
    Operation {
        id: format!("createFolder:{}", folder.display()),
        kind: OperationKind::CreateFolder,
        source: folder.clone(),
        relative_path: names.clone(),
        destination: names,
        destination_path: Some(folder),
        planned: SourceState {
            size: 0,
            modified: 0,
            is_package: false,
            is_folder: false,
        },
        reason,
        ticked: true,
        error: None,
    }
}

fn planned(entry: &ScanEntry) -> SourceState {
    SourceState {
        size: entry.size,
        modified: entry.modified,
        is_package: entry.is_package,
        is_folder: false,
    }
}

pub(crate) fn names_below(root: &Path, path: &Path) -> Vec<String> {
    let below = path.strip_prefix(root).unwrap_or(path);
    below
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect()
}
