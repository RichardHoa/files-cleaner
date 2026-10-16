//! Turns a scan into a Plan: the Operations the user is asked to approve.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{Config, DuplicateHandling, TrashDestination};
use crate::duplicates;
use crate::protection::HOLDING_FOLDER_NAME;
use crate::scanner::{ScanEntry, ScanResult};

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
    pub operations: Vec<Operation>,
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
    /// The names from the Scan Root down to where the item will be once the
    /// Plan runs, its own name last.
    pub destination: Vec<String>,
    /// For a duplicate, the names from the Scan Root down to its Duplicate
    /// Group's Keeper, which stays where it is.
    pub keeper: Option<Vec<String>>,
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

/// Proposes trashing the non-Keeper copies of every Duplicate Group, as
/// the config's Duplicate Handling says.
pub fn build_plan(scan: &ScanResult, config: &Config) -> Plan {
    let mut operations = Vec::new();
    if config.duplicates == DuplicateHandling::Trash {
        for group in duplicates::find(&scan.entries) {
            let keeper = &group.keeper.relative_path;
            let reason = format!("Duplicate of {}, which is kept", keeper.join("/"));
            for copy in group.copies {
                operations.push(trash(copy, keeper, reason.clone()));
            }
        }
    }
    operations.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Plan {
        root: scan.root.clone(),
        trash_destination: config.trash_destination,
        operations,
    }
}

/// Trashes `entry`, a copy of `keeper`, into this Session's folder in the
/// Holding Folder, keeping its path from the Scan Root.
fn trash(entry: &ScanEntry, keeper: &[String], reason: String) -> Operation {
    let destination = [HOLDING_FOLDER_NAME, THIS_SESSION_FOLDER]
        .into_iter()
        .map(String::from)
        .chain(entry.relative_path.iter().cloned())
        .collect();
    Operation {
        id: format!("trash:{}", entry.relative_path.join("/")),
        kind: OperationKind::Trash,
        source: entry.path.clone(),
        relative_path: entry.relative_path.clone(),
        destination,
        keeper: Some(keeper.to_vec()),
        planned: SourceState { size: entry.size, modified: entry.modified, is_package: entry.is_package },
        reason,
    }
}
