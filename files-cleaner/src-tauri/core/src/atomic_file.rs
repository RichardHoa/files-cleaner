//! Replacing a small file so a crash or a power cut leaves either the old
//! contents or the new, never an empty or half-written file.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

/// Tells apart the temporary files of writes in this process.
static WRITES: AtomicU64 = AtomicU64::new(0);

/// Replaces the file at `path` with `bytes`: written to a temporary file
/// next to it and flushed to disk, then renamed over it, then the folder
/// flushed too so the rename itself survives a power cut. Each write has
/// its own temporary file, so two at once each install a whole file.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    temporary.push(format!(".{}-{write}.tmp", process::id()));
    let temporary = Path::new(&temporary);
    let written = File::create(temporary).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(e) = written.and_then(|()| fs::rename(temporary, path)) {
        let _ = fs::remove_file(temporary);
        return Err(e);
    }
    match path.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => {
            remove_left_over(folder, path);
            sync_folder(folder)
        }
        _ => Ok(()),
    }
}

/// Deletes the temporary files of writes to `path` that a crash cut short.
/// A write takes moments, so one an hour old is never still going.
fn remove_left_over(folder: &Path, path: &Path) {
    const LEFT_OVER_AFTER: Duration = Duration::from_secs(60 * 60);
    let (Some(name), Ok(children)) = (path.file_name(), fs::read_dir(folder)) else {
        return;
    };
    let prefix = format!("{}.", name.to_string_lossy());
    for child in children.flatten() {
        let child_name = child.file_name();
        let child_name = child_name.to_string_lossy();
        if !(child_name.starts_with(&prefix) && child_name.ends_with(".tmp")) {
            continue;
        }
        let old = child
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > LEFT_OVER_AFTER);
        if old {
            let _ = fs::remove_file(child.path());
        }
    }
}

/// Waits until the names in `folder` are on disk, not just the files'
/// contents. Windows can't open a folder to sync it, and doesn't need to.
fn sync_folder(folder: &Path) -> io::Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    // Not path traversal: the folder is only flushed, and its path comes
    // from the app's own files, not from untrusted input.
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    File::open(folder)?.sync_all()
}
