//! The Linux System Trash: the FreeDesktop Trash layout
//! (https://specifications.freedesktop.org/trash/latest/). The info file goes
//! first, created atomically, then the item is renamed into `files/`. It never
//! copies to the home Trash from another disk: it refuses, as the spec allows.

use std::env;
use std::ffi::CString;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

use crate::executor::move_without_overwriting;

pub const PLACE: &str = "Trash";

fn uid() -> u32 {
    // SAFETY: geteuid has no preconditions and can't fail.
    unsafe { libc::geteuid() }
}

fn home_trash() -> Result<PathBuf, String> {
    let data = env::var_os("XDG_DATA_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or("Neither XDG_DATA_HOME nor HOME is set, so there is no Trash to use")?;
    Ok(data.join("Trash"))
}

/// The device of `path`, or of the nearest folder above it that exists.
fn device(path: &Path) -> Option<u64> {
    path.ancestors()
        .find_map(|p| fs::symlink_metadata(p).ok())
        .map(|m| m.dev())
}

/// The folder at the top of the mount holding `path`, which is `$topdir` in
/// the spec's words.
fn top_dir(path: &Path) -> Option<PathBuf> {
    let dev = device(path)?;
    path.ancestors()
        .take_while(|p| fs::symlink_metadata(p).is_ok_and(|m| m.dev() == dev))
        .last()
        .map(Path::to_path_buf)
}

/// `$topdir/.Trash/$uid`, if `$topdir/.Trash` passes the spec's checks: a real
/// folder, not a symlink, with the sticky bit set.
fn shared_trash(top: &Path) -> Option<PathBuf> {
    let shared = top.join(".Trash");
    let meta = fs::symlink_metadata(&shared).ok()?;
    let usable = meta.file_type().is_dir() && meta.mode() & libc::S_ISVTX != 0;
    usable.then(|| shared.join(uid().to_string()))
}

fn own_trash(top: &Path) -> PathBuf {
    top.join(format!(".Trash-{}", uid()))
}

fn make_trash_folders(trash: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder.create(trash.join("files"))?;
    builder.create(trash.join("info"))
}

/// Where an item in `parent` is trashed: the Trash folder, and the folder its
/// recorded paths are relative to (none for the home Trash, which uses
/// absolute paths).
fn find_trash(parent: &Path) -> Result<(PathBuf, Option<PathBuf>), String> {
    let home = home_trash()?;
    if device(&home) == device(parent) {
        make_trash_folders(&home)
            .map_err(|e| format!("Couldn't create {}: {e}", home.display()))?;
        return Ok((home, None));
    }
    let top = top_dir(parent).ok_or("Couldn't find the top of this disk")?;
    if let Some(shared) = shared_trash(&top) {
        if make_trash_folders(&shared).is_ok() {
            return Ok((shared, Some(top)));
        }
    }
    let own = own_trash(&top);
    if fs::symlink_metadata(&own).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("{} is a symlink, so it isn't used", own.display()));
    }
    make_trash_folders(&own)
        .map_err(|e| format!("This disk has no usable Trash: {}: {e}", own.display()))?;
    Ok((own, Some(top)))
}

/// Tells without creating anything, unlike `find_trash`.
pub fn blocked(folder: &Path) -> Option<String> {
    if home_trash().is_ok_and(|home| device(&home) == device(folder)) {
        return None;
    }
    let Some(top) = top_dir(folder) else {
        return Some("Couldn't find the top of this disk".to_string());
    };
    let can_create = CString::new(top.as_os_str().as_bytes())
        // SAFETY: `c` is a valid NUL-terminated path for the call's duration.
        .is_ok_and(|c| unsafe { libc::access(c.as_ptr(), libc::W_OK) } == 0);
    let usable = shared_trash(&top).is_some() || own_trash(&top).is_dir() || can_create;
    (!usable).then(|| "This disk has no usable Trash".to_string())
}

/// Percent-encodes as RFC 2396 says, keeping `/`.
fn encode_path(path: &Path) -> String {
    let mut out = String::new();
    for &byte in path.as_os_str().as_bytes() {
        match byte {
            b'/' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                out.push(byte as char)
            }
            _ if byte.is_ascii_alphanumeric() => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn deletion_date() -> String {
    // SAFETY: a null argument to `time` is allowed, `tm` is a valid out-pointer
    // for `localtime_r`, and zeroed is a valid `libc::tm`.
    let tm = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
    )
}

/// Claims a free name by creating its info file exclusively. The name must
/// also be free in `files/`, which may hold items whose info file went missing.
fn reserve_name(trash: &Path, name: &str, record: &str) -> Result<(PathBuf, PathBuf), String> {
    for attempt in 1..=10_000 {
        let candidate = if attempt == 1 {
            name.to_string()
        } else {
            format!("{name}.{attempt}")
        };
        let info = trash.join("info").join(format!("{candidate}.trashinfo"));
        let landing = trash.join("files").join(&candidate);
        if fs::symlink_metadata(&landing).is_ok() {
            continue;
        }
        match OpenOptions::new().write(true).create_new(true).open(&info) {
            Ok(mut file) => {
                return file
                    .write_all(record.as_bytes())
                    .map(|()| (info.clone(), landing))
                    .map_err(|e| {
                        let _ = fs::remove_file(&info);
                        format!("Couldn't write {}: {e}", info.display())
                    });
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Couldn't create {}: {e}", info.display())),
        }
    }
    Err(format!("No free name for {name} in {}", trash.display()))
}

pub fn trash(path: &Path) -> Result<PathBuf, String> {
    let parent = path.parent().ok_or("It has no parent folder")?;
    let name = path.file_name().ok_or("It has no name")?.to_string_lossy();
    let (trash, top) = find_trash(parent)?;
    let recorded = match &top {
        Some(top) => path.strip_prefix(top).map_err(|e| e.to_string())?,
        None => path,
    };
    let record = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        encode_path(recorded),
        deletion_date(),
    );
    let (info, landing) = reserve_name(&trash, &name, &record)?;
    if let Err(e) = fs::rename(path, &landing) {
        let _ = fs::remove_file(&info);
        return Err(format!("Couldn't move it to {}: {e}", landing.display()));
    }
    Ok(landing)
}

pub fn put_back(landed: &Path, to: &Path) -> Result<(), String> {
    move_without_overwriting(landed, to)?;
    forget(landed);
    Ok(())
}

/// Removes the item's info file. One left behind only clutters the Trash's
/// own bookkeeping, so a failure here doesn't undo a restore.
pub fn forget(landed: &Path) {
    let trash = landed.parent().and_then(Path::parent);
    if let (Some(trash), Some(name)) = (trash, landed.file_name()) {
        let mut info = name.to_os_string();
        info.push(".trashinfo");
        let _ = fs::remove_file(trash.join("info").join(info));
    }
}
