//! Turns a scan into a Plan: the Operations the user is asked to approve,
//! and a before/after tree of them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{Config, DuplicateHandling, TrashDestination};
use crate::duplicates;
use crate::protection::HOLDING_FOLDER_NAME;
use crate::scanner::{ScanEntry, ScanResult};

/// The name the after tree gives the Session's folder inside the Holding
/// Folder, which is only named once the Plan runs.
pub const THIS_SESSION_FOLDER: &str = "This Session";

/// The proposed Operations for one Scan Root.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// The Scan Root, with symlinks resolved.
    pub root: PathBuf,
    pub trash_destination: TrashDestination,
    pub operations: Vec<Operation>,
    /// Where each Operation's item is now, under the Scan Root.
    pub before: TreeNode,
    /// Where each Operation's item will be once the Plan runs.
    pub after: TreeNode,
}

/// One proposed file-system change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    /// The same for the same change every time a Plan is built.
    pub id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    /// The names from the Scan Root down to the source, its own name last.
    pub relative_path: Vec<String>,
    /// The source when scanned; it is skipped if it has changed when the
    /// Plan runs.
    #[serde(flatten)]
    pub planned: SourceState,
    /// Why the Plan proposes it, for the user.
    pub reason: String,
}

/// What an Operation's source was like when planned: if it differs when
/// the Operation runs, or is redone, the Operation is skipped.
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
}

/// A folder, or an item one Operation acts on.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub name: String,
    /// Folders first, then items, each sorted by name.
    pub children: Vec<TreeNode>,
    /// Set on an item: the Operation acting on it.
    pub operation_id: Option<String>,
    /// Every Operation at or under this node, so a folder can be ticked or
    /// unticked as a whole.
    pub operation_ids: Vec<String>,
}

/// Proposes trashing the non-Keeper copies of every Duplicate Group, as
/// the config's Duplicate Handling says.
pub fn build_plan(scan: &ScanResult, config: &Config) -> Plan {
    let mut operations = Vec::new();
    if config.duplicates == DuplicateHandling::Trash {
        for group in duplicates::find(&scan.entries) {
            let keeper = group.keeper.relative_path.join("/");
            for copy in group.copies {
                operations.push(trash(copy, format!("Duplicate of {keeper}, which is kept")));
            }
        }
    }
    operations.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let root_name = scan.root.file_name().unwrap_or(scan.root.as_os_str()).to_string_lossy().into_owned();
    let mut before = TreeNode { name: root_name.clone(), ..TreeNode::default() };
    let mut after = TreeNode { name: root_name, ..TreeNode::default() };
    for op in &operations {
        before.insert(&op.relative_path, &op.id);
        let destination: Vec<String> = [HOLDING_FOLDER_NAME, THIS_SESSION_FOLDER]
            .into_iter()
            .map(String::from)
            .chain(op.relative_path.iter().cloned())
            .collect();
        after.insert(&destination, &op.id);
    }
    before.sort();
    after.sort();

    Plan {
        root: scan.root.clone(),
        trash_destination: config.trash_destination,
        operations,
        before,
        after,
    }
}

fn trash(entry: &ScanEntry, reason: String) -> Operation {
    Operation {
        id: format!("trash:{}", entry.relative_path.join("/")),
        kind: OperationKind::Trash,
        source: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        planned: SourceState { size: entry.size, modified: entry.modified, is_package: entry.is_package },
        reason,
    }
}

impl TreeNode {
    /// Adds the item at `path` (below this node) for Operation `id`.
    fn insert(&mut self, path: &[String], id: &str) {
        self.operation_ids.push(id.to_string());
        let Some((name, rest)) = path.split_first() else {
            self.operation_id = Some(id.to_string());
            return;
        };
        let index = match self.children.iter().position(|c| &c.name == name) {
            Some(i) => i,
            None => {
                self.children.push(TreeNode { name: name.clone(), ..TreeNode::default() });
                self.children.len() - 1
            }
        };
        self.children[index].insert(rest, id);
    }

    fn sort(&mut self) {
        self.children.sort_by(|a, b| {
            (a.operation_id.is_some(), &a.name).cmp(&(b.operation_id.is_some(), &b.name))
        });
        for child in &mut self.children {
            child.sort();
        }
    }
}
