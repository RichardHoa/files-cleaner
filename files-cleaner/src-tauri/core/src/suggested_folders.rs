//! Turns the user's choices on the review screen, for each Duplicate
//! Group and each Suggested Folder, into one Plan.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clash;
use crate::config::Config;
use crate::folder_name::safe_folder_name;
use crate::planner::{finish, move_to, names_below, trash, Plan, Reason};
use crate::scanner::{ScanEntry, ScanResult};

/// What the user chose for one Suggested Folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedFolderChoice {
    /// As the user left it; made safe on every OS before planning.
    pub name: String,
    pub choice: FolderChoice,
    /// Its files, by scanned path, each with what happens to it.
    pub files: Vec<(PathBuf, FileChoice)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FolderChoice {
    /// A new folder in the Scan Root.
    CreateHere,
    /// Into `folder`, which already exists, with no Create Folder.
    AddTo { folder: PathBuf },
    /// A new folder inside `folder`, which the user picked.
    CreateIn { folder: PathBuf },
    /// Every file is trashed.
    TrashAll,
    /// Every file stays where it is.
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileChoice {
    /// Goes wherever its Suggested Folder's choice sends it.
    InFolder,
    Trash,
    /// Stays where it is.
    TakeOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestedFolderError {
    /// Nothing is left of `name` once made safe; it is never read as "trash".
    EmptyName { name: String },
}

impl fmt::Display for SuggestedFolderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SuggestedFolderError::EmptyName { name } if name.trim().is_empty() => {
                write!(f, "A Suggested Folder needs a name.")
            }
            SuggestedFolderError::EmptyName { name } => {
                write!(f, "\u{201c}{name}\u{201d} can't be a folder name on every computer. Try another name.")
            }
        }
    }
}

impl std::error::Error for SuggestedFolderError {}

/// What the user chose for one Duplicate Group: the copy that stays, and
/// the copies to trash. A copy in neither stays where it is.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateChoice {
    pub keeper: PathBuf,
    pub trash: Vec<PathBuf>,
}

/// Proposes each Suggested Folder's choice, as `build_review_plan` does
/// with no duplicates.
pub fn build_suggested_folders_plan(
    scan: &ScanResult,
    config: &Config,
    folders: &[SuggestedFolderChoice],
) -> Result<Plan, SuggestedFolderError> {
    build_review_plan(scan, config, &[], folders)
}

/// Proposes the whole review as one Plan. Each Duplicate Group's copies to
/// trash are trashed, naming its Keeper. Each Suggested Folder's choice is
/// a new folder (with a Clash Suffix if its name is taken) and a Move per
/// file into it, a Move per file into an existing folder, a Trash per file,
/// or nothing. A file the user trashes on its own is trashed whatever the
/// folder's choice, unless the folder is skipped; a file taken out stays
/// put. A copy already trashed as a duplicate is left out of its folder,
/// as are files the scan didn't find. Emptied folders, worked out from all
/// of it, and moves to another disk are handled as in `build_plan`.
pub fn build_review_plan(
    scan: &ScanResult,
    config: &Config,
    duplicates: &[DuplicateChoice],
    folders: &[SuggestedFolderChoice],
) -> Result<Plan, SuggestedFolderError> {
    let root = &scan.root;
    let entries: HashMap<&Path, &ScanEntry> =
        scan.entries.iter().map(|e| (e.path.as_path(), e)).collect();
    let mut operations = Vec::new();
    let mut trashed: HashSet<&Path> = HashSet::new();
    for group in duplicates {
        let reason = Reason::Duplicate {
            keeper: names_below(root, &group.keeper),
        };
        for path in &group.trash {
            if let Some(entry) = entries.get(path.as_path()) {
                trashed.insert(path);
                operations.push(trash(root, entry, reason.clone()));
            }
        }
    }
    // The names taken in each destination folder, lowercased.
    let mut taken: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let mut claim = |folder: &Path, name: &str, free: fn(&str, &HashSet<String>) -> String| {
        let names = taken
            .entry(folder.to_path_buf())
            .or_insert_with(|| clash::names_in(folder));
        let name = free(name, names);
        names.insert(name.to_lowercase());
        name
    };
    let mut created = Vec::new();
    for folder in folders {
        if folder.choice == FolderChoice::Skip {
            continue;
        }
        let name =
            safe_folder_name(&folder.name).ok_or_else(|| SuggestedFolderError::EmptyName {
                name: folder.name.clone(),
            })?;
        let destination = match &folder.choice {
            FolderChoice::Skip | FolderChoice::TrashAll => None,
            FolderChoice::AddTo { folder } => Some(folder.clone()),
            FolderChoice::CreateHere | FolderChoice::CreateIn { .. } => {
                let parent = match &folder.choice {
                    FolderChoice::CreateIn { folder } => folder.as_path(),
                    _ => root.as_path(),
                };
                let new = parent.join(claim(parent, &name, clash::free_folder_name));
                created.push((new.clone(), suggested(&new, &name)));
                Some(new)
            }
        };
        let reason = destination.as_deref().map_or_else(
            || Reason::SuggestedFolder {
                folder: name.clone(),
            },
            |d| suggested(d, &name),
        );
        for (path, choice) in &folder.files {
            let Some(entry) = entries.get(path.as_path()) else {
                continue;
            };
            if trashed.contains(path.as_path()) {
                continue;
            }
            match (choice, &destination) {
                (FileChoice::TakeOut, _) => {}
                (FileChoice::Trash, _) | (FileChoice::InFolder, None) => {
                    operations.push(trash(root, entry, reason.clone()))
                }
                (FileChoice::InFolder, Some(destination)) => {
                    let file_name = claim(destination, &entry.name, clash::free_name);
                    operations.push(move_to(
                        root,
                        entry,
                        destination.join(file_name),
                        reason.clone(),
                    ));
                }
            }
        }
    }
    Ok(finish(root, config, operations, created))
}

/// The Reason for a Suggested Folder whose files go to `destination`: named
/// as that folder is on disk, or `name` if it has no name of its own.
fn suggested(destination: &Path, name: &str) -> Reason {
    let folder = destination
        .file_name()
        .map_or_else(|| name.to_string(), |n| n.to_string_lossy().into_owned());
    Reason::SuggestedFolder { folder }
}
