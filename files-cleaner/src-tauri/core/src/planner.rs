//! Turns a scan into a Plan: the Operations the user is asked to approve.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clash;
use crate::config::{Config, Rule, RuleOperation, RuleTarget};
use crate::duplicates::DuplicateProgress;
use crate::os_trash::OsTrash;
use crate::protection::{is_os_metadata_file, is_package, is_within, same_path, Protection};
use crate::scanner::{ScanEntry, ScanResult};
use crate::volume::different_disks;
use crate::{duplicates, rules};

/// The proposed Operations for one Scan Root.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// The Scan Root, with symlinks resolved.
    pub root: PathBuf,
    /// Create Folder Operations first, each folder before the ones inside
    /// it, so a Plan run in order has every folder before it is needed;
    /// Trash of the folders the Plan empties last, once they are empty.
    pub operations: Vec<Operation>,
}

/// One proposed file-system change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    /// The same for the same change every time a Plan is built.
    pub id: String,
    pub kind: OperationKind,
    /// The item it changes; for Create Folder, the folder it creates.
    pub source: PathBuf,
    /// The names from the Scan Root down to the source, its own name last.
    /// A source outside the Scan Root is given in full.
    pub relative_path: Vec<String>,
    /// The names from the Scan Root down to where the item will be once the
    /// Plan runs, its own name last. A destination outside the Scan Root is
    /// given in full. Empty for Trash, which goes to the System Trash.
    pub destination: Vec<String>,
    /// Where the item will be once the Plan runs. `None` for Trash: only the
    /// OS knows where it lands, and only once it has.
    pub destination_path: Option<PathBuf>,
    /// The source when scanned; it is skipped if it has changed when the
    /// Plan runs.
    #[serde(flatten)]
    pub planned: SourceState,
    /// Why the Plan proposes it.
    pub reason: Reason,
    /// Whether it starts ticked. Trashing an emptied folder starts
    /// unticked, as does an Operation that can't happen.
    pub ticked: bool,
    /// Why it can't happen, such as a move to another disk or a Trash on a
    /// disk with no System Trash. Such an Operation can't be ticked, and is
    /// never run; a blocked Trash that is approved anyway is journaled as
    /// skipped, with this as its Note.
    pub error: Option<String>,
}

/// What an Operation's source was like when planned: if it differs when
/// the Operation runs, or is redone, the Operation is skipped. A folder to
/// create has none of its own, so it is all zeros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceState {
    /// Bytes; for a Package, the total of everything inside it.
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub modified: i64,
    pub is_package: bool,
    /// An ordinary folder the Plan empties: it is trashed only if, when the
    /// time comes, it holds nothing but OS Metadata Files and folders that
    /// hold nothing either.
    #[serde(default)]
    pub is_folder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    Trash,
    /// To another folder, under a new name if its Rule renames it or its
    /// name was taken there.
    Move,
    CreateFolder,
}

/// Why the Plan proposes an Operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Reason {
    /// The item is a copy of `kept`, the names from the Scan Root down to a
    /// Kept Copy of its Duplicate Group, or `None` if no copy is kept.
    Duplicate { kept: Option<Vec<String>> },
    /// The Rule named `rule` matched the item, or, for Create Folder, needs
    /// the folder.
    Rule { rule: String },
    /// Every item in the folder leaves it, so the folder would be left
    /// empty.
    EmptiedFolder,
    /// The item goes into, or is trashed from, the Suggested Folder named
    /// `folder`, or, for Create Folder, is that folder.
    SuggestedFolder { folder: String },
}

/// Proposes the duplicate step: every copy but the first in a Duplicate Group
/// is trashed, and nothing else. A folder the Plan would empty is offered
/// for Trash, unticked. A Move that would cross from one disk to another,
/// or a Trash `os_trash` says can't happen, is kept, as an error that can't be
/// ticked.
pub fn build_plan(scan: &ScanResult, config: &Config, os_trash: &dyn OsTrash) -> Plan {
    build_plan_with_progress(scan, config, os_trash, |_| {})
}

/// Like `build_plan`, calling `on_progress` while finding duplicates (see
/// `duplicates::find`).
pub fn build_plan_with_progress(
    scan: &ScanResult,
    config: &Config,
    os_trash: &dyn OsTrash,
    on_progress: impl FnMut(DuplicateProgress),
) -> Plan {
    let root = &scan.root;
    let mut operations = Vec::new();
    for group in duplicates::find(&scan.entries, config.default_copy_rule, on_progress) {
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

/// Rules are frozen: no Plan the app builds uses them, but the code stays
/// for if they come back. This proposes what the first Rule each scanned
/// item matches says, treating duplicate copies like any other file. Rules
/// move items into folders, which are created if missing, and never over
/// anything: a taken name gets a Clash Suffix. An item already in its
/// Rule's folder stays put. Emptied folders and Operations that can't happen
/// are handled as in `build_plan`.
pub fn build_rule_plan(scan: &ScanResult, config: &Config, os_trash: &dyn OsTrash) -> Plan {
    let root = &scan.root;
    let protection = Protection::new(config);
    let mut operations = Vec::new();
    let mut folders = Folders::default();
    for entry in &scan.entries {
        let Some(rule) = rules::first_match(&config.rules, entry) else {
            continue;
        };
        let reason = Reason::Rule {
            rule: rule.name.clone(),
        };
        match &rule.operation {
            RuleOperation::Trash => operations.push(trash(entry, reason)),
            RuleOperation::MoveTo { folder, rename } => {
                let folder = match folder {
                    RuleTarget::Absolute(folder) => folder.clone(),
                    RuleTarget::RelativeToScanRoot(folder) => {
                        let folder = root.join(folder);
                        if protected_below(&protection, &folder, root) {
                            continue;
                        }
                        folder
                    }
                };
                if entry
                    .path
                    .parent()
                    .is_some_and(|parent| same_path(parent, &folder))
                {
                    continue;
                }
                let name = match rename {
                    Some(template) => rules::rename(template, entry),
                    None => entry.name.clone(),
                };
                let name = folders.claim(&folder, &name, rule);
                operations.push(move_to(root, entry, folder.join(name), reason));
            }
        }
    }
    finish(root, os_trash, operations, folders.created)
}

/// Makes a Plan of `operations` and of creating the folders in `created`,
/// each with the Reason it is needed: in order, with the folders the Plan
/// empties offered for Trash, and moves to another disk and Trash that
/// `os_trash` says can't happen marked as errors.
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

/// Why `op` can't happen, if it would move its item to another disk.
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

/// The folders inside `root` that `operations` would leave empty, apart
/// from OS Metadata Files: every item in them leaves, and nothing arrives.
/// Only the topmost of nested ones is given, as trashing it takes the
/// others along.
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

/// True if everything in `folder` is `leaving`, an OS Metadata File, or a
/// folder that `empties` too, and nothing in `arriving` goes into it.
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

/// The folders Rules move items into: the names taken in each, and the
/// missing ones to create, with the first Rule that needs each.
#[derive(Default)]
struct Folders {
    taken: HashMap<PathBuf, HashSet<String>>,
    created: Vec<(PathBuf, Reason)>,
}

impl Folders {
    /// Takes a free name in `folder` for `name`, a file `rule` moves there,
    /// and notes the folders to create on the way.
    fn claim(&mut self, folder: &Path, name: &str, rule: &Rule) -> String {
        if !self.taken.contains_key(folder) {
            for missing in folder.ancestors().take_while(|f| !f.exists()) {
                if !self.created.iter().any(|(f, _)| f == missing) {
                    self.created.push((
                        missing.to_path_buf(),
                        Reason::Rule {
                            rule: rule.name.clone(),
                        },
                    ));
                }
            }
        }
        let taken = self
            .taken
            .entry(folder.to_path_buf())
            .or_insert_with(|| clash::names_in(folder));
        let name = clash::free_name(name, taken);
        taken.insert(name.to_lowercase());
        name
    }
}

/// True if `folder`, or a folder between it and the Scan Root `root`, is a
/// Protected Path or a Package. The Scan Root and the folders above it
/// don't count: the user chose to scan it.
fn protected_below(protection: &Protection, folder: &Path, root: &Path) -> bool {
    folder
        .ancestors()
        .take_while(|f| is_within(f, root) && !same_path(f, root))
        .any(|f| protection.reason(f, true).is_some() || is_package(f))
}

/// Sends `entry` to the System Trash.
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

/// Trashes `folder`, which the Plan empties, like any other item; it starts
/// unticked.
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

/// The names from `root` down to `path`, its own name last. A path outside
/// `root` is given in full.
pub(crate) fn names_below(root: &Path, path: &Path) -> Vec<String> {
    let below = path.strip_prefix(root).unwrap_or(path);
    below
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect()
}
