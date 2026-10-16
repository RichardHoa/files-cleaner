//! Turns a scan into a Plan: the Operations the user is asked to approve.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clash;
use crate::config::{Config, DuplicateHandling, Rule, RuleOperation, RuleTarget, TrashDestination};
use crate::protection::{is_package, is_within, same_path, Protection, HOLDING_FOLDER_NAME};
use crate::scanner::{ScanEntry, ScanResult};
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
    /// it, so a Plan run in order has every folder before it is needed.
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
}

/// Proposes what to do with each scanned item: a non-Keeper copy in a
/// Duplicate Group is trashed or left alone if Duplicate Handling says so;
/// anything else gets the first Rule it matches, if any. Rules move items
/// into folders, which are created if missing, and never over anything:
/// a taken name gets a Clash Suffix. An item already in its Rule's folder
/// stays put.
pub fn build_plan(scan: &ScanResult, config: &Config) -> Plan {
    let root = &scan.root;
    let protection = Protection::new(config);
    let mut copies: HashMap<&Path, &[String]> = HashMap::new();
    if config.duplicates != DuplicateHandling::FollowRules {
        for group in duplicates::find(&scan.entries) {
            for copy in group.copies {
                copies.insert(&copy.path, &group.keeper.relative_path);
            }
        }
    }

    let mut operations = Vec::new();
    let mut folders = Folders::default();
    for entry in &scan.entries {
        if let Some(keeper) = copies.get(entry.path.as_path()) {
            if config.duplicates == DuplicateHandling::Trash {
                operations.push(trash(root, entry, Reason::Duplicate { keeper: keeper.to_vec() }));
            }
            continue;
        }
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
    operations.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    // Sorted by path, each folder comes before the folders inside it.
    let mut created = folders.created;
    created.sort_by(|a, b| a.0.cmp(&b.0));
    let created = created.into_iter().map(|(folder, rule)| create_folder_operation(root, folder, rule));
    Plan {
        root: root.clone(),
        trash_destination: config.trash_destination,
        operations: created.chain(operations).collect(),
    }
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
    let destination: Vec<String> = [HOLDING_FOLDER_NAME, THIS_SESSION_FOLDER]
        .into_iter()
        .map(String::from)
        .chain(entry.relative_path.iter().cloned())
        .collect();
    Operation {
        id: format!("trash:{}", entry.relative_path.join("/")),
        kind: OperationKind::Trash,
        source: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        destination_path: root.join(destination.iter().collect::<PathBuf>()),
        destination,
        planned: planned(entry),
        reason,
    }
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
        planned: SourceState { size: 0, modified: 0, is_package: false },
        reason: Reason::Rule { rule },
    }
}

fn planned(entry: &ScanEntry) -> SourceState {
    SourceState { size: entry.size, modified: entry.modified, is_package: entry.is_package }
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
