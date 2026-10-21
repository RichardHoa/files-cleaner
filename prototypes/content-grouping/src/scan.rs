//! Which items of a Scan Root the prototype groups: only loose files directly
//! in it. Existing subfolders are possible destinations, not inputs.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A loose file the groupers see.
#[derive(Debug, Clone)]
pub struct Item {
    pub path: PathBuf,
    pub name: String,
    /// Lowercased, without the dot.
    pub ext: String,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Skip {
    Hidden,
    OsMetadata,
    NoExtension,
    Package,
    Symlink,
    /// The app's own Holding Folder (`Files Cleaner Trash`).
    HoldingFolder,
    /// Gone or not readable by the time we looked (permissions, a race).
    Unreadable,
}

impl Skip {
    pub fn label(self) -> &'static str {
        match self {
            Skip::Hidden => "hidden",
            Skip::OsMetadata => "OS Metadata Files",
            Skip::NoExtension => "no extension",
            Skip::Package => "Packages",
            Skip::Symlink => "symlinks",
            Skip::HoldingFolder => "Holding Folder",
            Skip::Unreadable => "unreadable",
        }
    }
}

#[derive(Debug, Default)]
pub struct Listing {
    pub loose: Vec<Item>,
    pub subfolders: Vec<PathBuf>,
    pub skipped: Vec<(PathBuf, Skip)>,
}

/// Copied from the core (`protection/app_files.rs`): every OS's metadata
/// files, on every OS.
const OS_METADATA_FILES: &[&str] = &[".DS_Store", "Icon\r", ".localized", "Thumbs.db", "desktop.ini", ".directory"];

/// Copied from the core (`protection/mod.rs`): Package extensions known
/// regardless of the OS. Only folders can be Packages; a flat `.pkg`
/// installer file is an ordinary loose file.
const PACKAGE_EXTENSIONS: &[&str] = &[
    "app", "bundle", "framework", "plugin", "kext", "appex", "xpc", "prefpane", "qlgenerator",
    "mdimporter", "saver", "component", "pkg", "mpkg", "pages", "numbers", "key", "rtfd",
    "xcodeproj", "xcworkspace", "playground", "photoslibrary", "musiclibrary", "tvlibrary",
    "imovielibrary", "fcpbundle", "logicx", "band", "lrdata", "scriv", "sparsebundle",
];

const HOLDING_FOLDER_NAME: &str = "Files Cleaner Trash";

fn extension(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && !ext.is_empty()).then(|| ext.to_lowercase())
}

#[cfg(windows)]
fn has_hidden_attribute(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn has_hidden_attribute(_meta: &std::fs::Metadata) -> bool {
    false
}

enum Entry {
    File(Item),
    Folder(PathBuf),
    Skipped(PathBuf, Skip),
}

/// Classifies one directory entry without following symlinks.
fn classify(path: PathBuf) -> std::io::Result<Entry> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let meta = std::fs::symlink_metadata(&path)?;
    let skip = if meta.file_type().is_symlink() {
        Some(Skip::Symlink)
    } else if OS_METADATA_FILES.contains(&name.as_str()) {
        Some(Skip::OsMetadata)
    } else if name.starts_with('.') || has_hidden_attribute(&meta) {
        Some(Skip::Hidden)
    } else if meta.is_dir() && name == HOLDING_FOLDER_NAME {
        Some(Skip::HoldingFolder)
    } else if meta.is_dir() && extension(&name).is_some_and(|e| PACKAGE_EXTENSIONS.contains(&e.as_str())) {
        Some(Skip::Package)
    } else if meta.is_file() && extension(&name).is_none() {
        Some(Skip::NoExtension)
    } else {
        None
    };
    Ok(match skip {
        Some(skip) => Entry::Skipped(path, skip),
        None if meta.is_dir() => Entry::Folder(path),
        None => Entry::File(Item {
            ext: extension(&name).unwrap_or_default(),
            name,
            // Some filesystems keep no modified date; the baseline's month split then sees 1970.
            modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            path,
        }),
    })
}

fn sorted_entries(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    // An entry that can't be listed is left out rather than failing the scan.
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    Ok(paths)
}

/// The loose files directly in `root`, its subfolders, and what was left out.
pub fn loose_items(root: &Path) -> std::io::Result<Listing> {
    let mut listing = Listing::default();
    for path in sorted_entries(root)? {
        match classify(path.clone()) {
            Ok(Entry::File(item)) => listing.loose.push(item),
            Ok(Entry::Folder(path)) => listing.subfolders.push(path),
            Ok(Entry::Skipped(path, skip)) => listing.skipped.push((path, skip)),
            Err(_) => listing.skipped.push((path, Skip::Unreadable)),
        }
    }
    Ok(listing)
}

/// Up to `cap` files anywhere inside `folder`, by the same rules as loose
/// items, breadth-first so a deep tree doesn't crowd out the top level.
pub fn sample_files(folder: &Path, cap: usize) -> Vec<Item> {
    let mut files = Vec::new();
    let mut queue = std::collections::VecDeque::from([folder.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(paths) = sorted_entries(&dir) else { continue };
        for path in paths {
            if files.len() >= cap {
                return files;
            }
            match classify(path) {
                Ok(Entry::File(item)) => files.push(item),
                Ok(Entry::Folder(path)) => queue.push_back(path),
                _ => {}
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn names(paths: impl IntoIterator<Item = PathBuf>) -> Vec<String> {
        let mut names: Vec<String> =
            paths.into_iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn groups_only_loose_files_and_offers_subfolders_as_destinations() {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        fs::write(r.join("Invoice March.pdf"), "x").unwrap();
        fs::write(r.join("notes.TXT"), "x").unwrap();
        fs::create_dir(r.join("Taxes")).unwrap();
        fs::write(r.join("Taxes").join("inside.pdf"), "x").unwrap();

        let listing = loose_items(r).unwrap();

        assert_eq!(names(listing.loose.iter().map(|i| i.path.clone())), ["Invoice March.pdf", "notes.TXT"]);
        assert_eq!(names(listing.subfolders), ["Taxes"]);
        let notes = listing.loose.iter().find(|i| i.name == "notes.TXT").unwrap();
        assert_eq!(notes.ext, "txt");
    }

    #[test]
    fn skips_hidden_metadata_extensionless_and_packages() {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        fs::write(r.join("keep.png"), "x").unwrap();
        fs::write(r.join(".secret.txt"), "x").unwrap();
        fs::write(r.join(".DS_Store"), "x").unwrap();
        fs::write(r.join("Thumbs.db"), "x").unwrap();
        fs::write(r.join("desktop.ini"), "x").unwrap();
        fs::write(r.join("README"), "x").unwrap();
        fs::create_dir(r.join("Pages Doc.pages")).unwrap();
        fs::create_dir(r.join("Some.app")).unwrap();
        fs::create_dir(r.join(".git")).unwrap();
        fs::create_dir(r.join("Files Cleaner Trash")).unwrap();

        let listing = loose_items(r).unwrap();

        assert_eq!(names(listing.loose.iter().map(|i| i.path.clone())), ["keep.png"]);
        assert!(listing.subfolders.is_empty(), "{:?}", listing.subfolders);
        let mut skipped: Vec<(String, Skip)> = listing
            .skipped
            .iter()
            .map(|(p, s)| (p.file_name().unwrap().to_string_lossy().into_owned(), *s))
            .collect();
        skipped.sort();
        assert_eq!(
            skipped,
            [
                (".DS_Store".into(), Skip::OsMetadata),
                (".git".into(), Skip::Hidden),
                (".secret.txt".into(), Skip::Hidden),
                ("Files Cleaner Trash".into(), Skip::HoldingFolder),
                ("Pages Doc.pages".into(), Skip::Package),
                ("README".into(), Skip::NoExtension),
                ("Some.app".into(), Skip::Package),
                ("Thumbs.db".into(), Skip::OsMetadata),
                ("desktop.ini".into(), Skip::OsMetadata),
            ]
        );
    }
}
