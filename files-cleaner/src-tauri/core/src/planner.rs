//! Turns a scan into a Plan: the Operations the user is asked to approve.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clash;
use crate::config::{Config, Rule, RuleOperation, RuleTarget, TrashDestination};
use crate::protection::{is_os_metadata_file, is_package, is_within, same_path, Protection, HOLDING_FOLDER_NAME};
use crate::scanner::{ScanEntry, ScanResult};
use crate::volume::different_disks;
use crate::{duplicates, rules};

/// The name a Plan's destinations give the Session's folder inside the
/// Holding Folder, which is only named once the Plan runs.
pub const THIS_SESSION_FOLDER: &str = "This Session";

/// The proposed Operations for one Scan Root.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// The Scan Root, with symlinks resolved.
    pub root: PathBuf,
    pub trash_destination: TrashDestination,
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
    /// given in full.
    pub destination: Vec<String>,
    /// Where the item will be once the Plan runs. For Trash, its Session's
    /// folder in the Holding Folder is only named then, so this holds
    /// `THIS_SESSION_FOLDER` instead.
    pub destination_path: PathBuf,
    /// The source when scanned; it is skipped if it has changed when the
    /// Plan runs.
    #[serde(flatten)]
    pub planned: SourceState,
    /// Why the Plan proposes it.
    pub reason: Reason,
    /// Whether it starts ticked. Trashing an emptied folder starts
    /// unticked, as does an Operation that can't happen.
    pub ticked: bool,
    /// Why it can't happen, such as a move to another disk. Such an
    /// Operation can't be ticked, and is never run.
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
    /// The item is a copy of `keeper`, the names from the Scan Root down to
    /// its Duplicate Group's Keeper, which stays where it is.
    Duplicate { keeper: Vec<String> },
    /// The Rule named `rule` matched the item, or, for Create Folder, needs
    /// the folder.
    Rule { rule: String },
    /// Every item in the folder leaves it, so the folder would be left
    /// empty.
    EmptiedFolder,
}

/// Proposes the duplicate step: every non-Keeper copy in a Duplicate Group
/// is trashed, and nothing else. A folder the Plan would empty is offered
/// for Trash, unticked. An Operation that would cross from one disk to
/// another is kept, as an error that can't be ticked.
pub fn build_plan(scan: &ScanResult, config: &Config) -> Plan {
    let root = &scan.root;
    let mut operations = Vec::new();
    for group in duplicates::find(&scan.entries) {
        for copy in group.copies {
            operations.push(trash(root, copy, Reason::Duplicate { keeper: group.keeper.relative_path.clone() }));
        }
    }
    finish(root, config, operations, Vec::new())
}

/// Rules are frozen: no Plan the app builds uses them, but the code stays
/// for if they come back. This proposes what the first Rule each scanned
/// item matches says, treating duplicate copies like any other file. Rules
/// move items into folders, which are created if missing, and never over
/// anything: a taken name gets a Clash Suffix. An item already in its
/// Rule's folder stays put. Emptied folders and moves to another disk are
/// handled as in `build_plan`.
pub fn build_rule_plan(scan: &ScanResult, config: &Config) -> Plan {
    let root = &scan.root;
    let protection = Protection::new(config);
    let mut operations = Vec::new();
    let mut folders = Folders::default();
    for entry in &scan.entries {
        let Some(rule) = rules::first_match(&config.rules, entry) else { continue };
        let reason = Reason::Rule { rule: rule.name.clone() };
        match &rule.operation {
            RuleOperation::Trash => operations.push(trash(root, entry, reason)),
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
                if entry.path.parent().is_some_and(|parent| same_path(parent, &folder)) {
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
    finish(root, config, operations, folders.created)
}

/// Makes a Plan of `operations` and of creating the folders in `created`,
/// each with the Rule that needs it: in order, with the folders the Plan
/// empties offered for Trash and moves to another disk marked as errors.
fn finish(root: &Path, config: &Config, mut operations: Vec<Operation>, mut created: Vec<(PathBuf, String)>) -> Plan {
    operations.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    // Sorted by path, each folder comes before the folders inside it.
    created.sort_by(|a, b| a.0.cmp(&b.0));
    let created = created.into_iter().map(|(folder, rule)| create_folder_operation(root, folder, rule));
    let mut operations: Vec<Operation> = created.chain(operations).collect();
    for op in &mut operations {
        op.error = across_disks(op);
    }
    let emptied = emptied_folders(root, &operations);
    for folder in emptied {
        let mut op = trash_emptied_folder(root, &folder);
        op.error = across_disks(&op);
        operations.push(op);
    }
    for op in &mut operations {
        op.ticked &= op.error.is_none();
    }
    Plan { root: root.to_path_buf(), trash_destination: config.trash_destination, operations }
}

/// Why `op` can't happen, if its item would move to another disk.
fn across_disks(op: &Operation) -> Option<String> {
    if op.kind == OperationKind::CreateFolder || !different_disks(&op.source, &op.destination_path) {
        return None;
    }
    let name = op.relative_path.last().map_or_else(|| op.source.display().to_string(), Clone::clone);
    let to = match op.kind {
        OperationKind::Trash => format!("the {HOLDING_FOLDER_NAME} folder"),
        _ => op.destination_path.parent().unwrap_or(&op.destination_path).display().to_string(),
    };
    Some(format!("{name} is on a different disk from {to}, and the app only moves items within one disk"))
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
        .filter(|op| op.kind != OperationKind::Trash)
        .map(|op| op.destination_path.as_path())
        .collect();
    // Sorted by path, each folder comes before the folders inside it.
    let candidates: BTreeSet<&Path> = leaving
        .iter()
        .flat_map(|source| source.ancestors().skip(1).take_while(|folder| folder.starts_with(root) && *folder != root))
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
fn empties(folder: &Path, leaving: &HashSet<&Path>, arriving: &[&Path], known: &mut HashMap<PathBuf, bool>) -> bool {
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
                    Ok(kind) if kind.is_file() => is_os_metadata_file(&child.file_name().to_string_lossy()),
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
    created: Vec<(PathBuf, String)>,
}

impl Folders {
    /// Takes a free name in `folder` for `name`, a file `rule` moves there,
    /// and notes the folders to create on the way.
    fn claim(&mut self, folder: &Path, name: &str, rule: &Rule) -> String {
        if !self.taken.contains_key(folder) {
            for missing in folder.ancestors().take_while(|f| !f.exists()) {
                if !self.created.iter().any(|(f, _)| f == missing) {
                    self.created.push((missing.to_path_buf(), rule.name.clone()));
                }
            }
        }
        let taken = self.taken.entry(folder.to_path_buf()).or_insert_with(|| clash::names_in(folder));
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

/// Trashes `entry` into this Session's folder in the Holding Folder,
/// keeping its path from the Scan Root.
fn trash(root: &Path, entry: &ScanEntry, reason: Reason) -> Operation {
    let destination = in_holding_folder(&entry.relative_path);
    Operation {
        id: format!("trash:{}", entry.relative_path.join("/")),
        kind: OperationKind::Trash,
        source: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        destination_path: root.join(destination.iter().collect::<PathBuf>()),
        destination,
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
    let destination = in_holding_folder(&relative_path);
    Operation {
        id: format!("trashFolder:{}", relative_path.join("/")),
        kind: OperationKind::Trash,
        source: folder.to_path_buf(),
        destination_path: root.join(destination.iter().collect::<PathBuf>()),
        relative_path,
        destination,
        planned: SourceState { size: 0, modified: 0, is_package: false, is_folder: true },
        reason: Reason::EmptiedFolder,
        ticked: false,
        error: None,
    }
}

/// The names from the Scan Root down to where Trash puts the item at
/// `relative_path`.
fn in_holding_folder(relative_path: &[String]) -> Vec<String> {
    [HOLDING_FOLDER_NAME, THIS_SESSION_FOLDER].into_iter().map(String::from).chain(relative_path.iter().cloned()).collect()
}

fn move_to(root: &Path, entry: &ScanEntry, destination: PathBuf, reason: Reason) -> Operation {
    Operation {
        id: format!("move:{}", entry.relative_path.join("/")),
        kind: OperationKind::Move,
        source: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        destination: names_below(root, &destination),
        destination_path: destination,
        planned: planned(entry),
        reason,
        ticked: true,
        error: None,
    }
}

fn create_folder_operation(root: &Path, folder: PathBuf, rule: String) -> Operation {
    let names = names_below(root, &folder);
    Operation {
        id: format!("createFolder:{}", folder.display()),
        kind: OperationKind::CreateFolder,
        source: folder.clone(),
        relative_path: names.clone(),
        destination: names,
        destination_path: folder,
        planned: SourceState { size: 0, modified: 0, is_package: false, is_folder: false },
        reason: Reason::Rule { rule },
        ticked: true,
        error: None,
    }
}

fn planned(entry: &ScanEntry) -> SourceState {
    SourceState { size: entry.size, modified: entry.modified, is_package: entry.is_package, is_folder: false }
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
