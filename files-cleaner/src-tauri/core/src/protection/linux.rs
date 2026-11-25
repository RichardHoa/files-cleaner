use std::fs::DirEntry;
use std::path::{Component, Path, PathBuf};

use super::same_path;

const SYSTEM_FOLDERS: &[&str] = &[
    "/usr", "/etc", "/bin", "/sbin", "/lib", "/lib64", "/boot", "/proc", "/sys", "/dev", "/run",
    "/var", "/opt", "/snap",
];

const HOME_SYSTEM_FOLDERS: &[&str] = &[".config", ".local", ".cache"];

/// With how many names below it a disk's own folder is.
const MOUNT_FOLDERS: &[(&str, usize)] = &[("/mnt", 1), ("/media", 2), ("/run/media", 2)];

/// Folders that only hold mounted disks count too.
pub fn is_disk_root(path: &Path) -> bool {
    if path == Path::new("/") {
        return true;
    }
    // Case-insensitive, like every Protected Path check.
    let path = PathBuf::from(path.to_string_lossy().to_lowercase());
    MOUNT_FOLDERS.iter().any(|(folder, depth)| {
        let Ok(below) = path.strip_prefix(folder) else {
            return false;
        };
        below
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
            && below.components().count() <= *depth
    })
}

/// The walk up from a path stops at its Disk Root, so disks under `/run/media`
/// aren't inside `/run`.
pub fn builtin_reason(path: &Path, home: &Path) -> Option<String> {
    let in_home = HOME_SYSTEM_FOLDERS.iter().map(|name| home.join(name));
    SYSTEM_FOLDERS
        .iter()
        .map(|folder| Path::new(folder).to_path_buf())
        .chain(in_home)
        .find(|folder| same_path(path, folder))
        .map(|folder| format!("{} is a System Folder", folder.display()))
}

/// A leading dot is all there is, and that is checked on every OS.
pub fn is_hidden(_entry: &DirEntry) -> bool {
    false
}

/// No OS-wide notion of a Package; the shared extension list covers them.
pub fn is_package(_folder: &Path) -> bool {
    false
}
