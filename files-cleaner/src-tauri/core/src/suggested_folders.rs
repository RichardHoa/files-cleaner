use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clash;
use crate::folder_name::safe_folder_name;
use crate::os_trash::OsTrash;
use crate::planner::{finish, move_to, names_below, trash, Plan, Reason};
use crate::scanner::{ScanEntry, ScanResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedFolderChoice {
    /// Not yet safe on every OS.
    pub name: String,
    pub choice: FolderChoice,
    pub files: Vec<(PathBuf, FileChoice)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FolderChoice {
    CreateHere,
    /// No Create Folder: it already exists.
    AddTo {
        folder: PathBuf,
    },
    CreateIn {
        folder: PathBuf,
    },
    TrashAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileChoice {
    InFolder,
    Trash,
    /// Stays where it is.
    TakeOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestedFolderError {
    /// An error, as an empty name must never be read as "trash".
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

/// A copy in neither list stays where it is.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateChoice {
    pub kept: Option<PathBuf>,
    pub trash: Vec<PathBuf>,
}

pub fn build_suggested_folders_plan(
    scan: &ScanResult,
    os_trash: &dyn OsTrash,
    folders: &[SuggestedFolderChoice],
) -> Result<Plan, SuggestedFolderError> {
    build_review_plan(scan, os_trash, &[], folders)
}

/// One Plan for everything, as Emptied Folders depend on all of it. A copy
/// already trashed as a duplicate is left out of its folder.
pub fn build_review_plan(
    scan: &ScanResult,
    os_trash: &dyn OsTrash,
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
            kept: group.kept.as_ref().map(|kept| names_below(root, kept)),
        };
        for path in &group.trash {
            if let Some(entry) = entries.get(path.as_path()) {
                trashed.insert(path);
                operations.push(trash(entry, reason.clone()));
            }
        }
    }
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
    // A `/` in names nests folders; each outer one is made once for all inside
    // it.
    let mut outer: HashMap<(PathBuf, String), PathBuf> = HashMap::new();
    for folder in folders {
        let empty = || SuggestedFolderError::EmptyName {
            name: folder.name.clone(),
        };
        let mut parts = folder
            .name
            .split('/')
            .map(safe_folder_name)
            .collect::<Option<Vec<String>>>()
            .ok_or_else(empty)?;
        let name = parts.pop().ok_or_else(empty)?;
        let destination = match &folder.choice {
            FolderChoice::TrashAll => None,
            FolderChoice::AddTo { folder } => Some(folder.clone()),
            FolderChoice::CreateHere | FolderChoice::CreateIn { .. } => {
                let mut parent = match &folder.choice {
                    FolderChoice::CreateIn { folder } => folder.clone(),
                    _ => root.clone(),
                };
                for part in &parts {
                    let key = (parent.clone(), part.to_lowercase());
                    parent = match outer.get(&key) {
                        Some(found) => found.clone(),
                        None => {
                            let found = existing_folder(&parent, part).unwrap_or_else(|| {
                                let new =
                                    parent.join(claim(&parent, part, clash::free_folder_name));
                                created.push((new.clone(), suggested(&new, part)));
                                new
                            });
                            outer.insert(key, found.clone());
                            found
                        }
                    };
                }
                let parent = parent.as_path();
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
                    operations.push(trash(entry, reason.clone()))
                }
                (FileChoice::InFolder, Some(destination))
                    if entry.path.parent() == Some(destination.as_path()) => {}
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
    Ok(finish(root, os_trash, operations, created))
}

fn existing_folder(parent: &Path, name: &str) -> Option<PathBuf> {
    let wanted = name.to_lowercase();
    fs::read_dir(parent)
        .ok()?
        .flatten()
        .find(|child| {
            child.file_name().to_string_lossy().to_lowercase() == wanted
                && child.file_type().is_ok_and(|t| t.is_dir())
        })
        .map(|child| child.path())
}

/// As on disk, so the Reason names the folder the user will find.
fn suggested(destination: &Path, name: &str) -> Reason {
    let folder = destination
        .file_name()
        .map_or_else(|| name.to_string(), |n| n.to_string_lossy().into_owned());
    Reason::SuggestedFolder { folder }
}
