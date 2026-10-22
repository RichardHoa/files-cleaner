#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use files_cleaner_core::{load_config, Config};
use tempfile::TempDir;

/// A temp folder for a test to scan.
///
/// It lives under the crate folder rather than the system temp dir: macOS
/// puts that under `/private`, and `target/` sits next to a `Cargo.toml`,
/// and both are Protected Paths that would hide the test's own files.
pub fn scan_root() -> TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(base)
        .unwrap()
}

/// `path` with symlinks resolved, in the form the app gives it: on Windows
/// `C:\…`, not the `\\?\C:\…` that `fs::canonicalize` returns.
pub fn real_path(path: &Path) -> PathBuf {
    let real = fs::canonicalize(path).unwrap();
    #[cfg(windows)]
    {
        if let Some(plain) = real.to_str().and_then(|s| s.strip_prefix(r"\\?\")).filter(|s| !s.starts_with(r"UNC\")) {
            return PathBuf::from(plain);
        }
    }
    real
}

/// Writes `contents` to `root/relative`, creating parent folders.
pub fn write(root: &Path, relative: &str, contents: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();
    path
}

pub fn config(text: &str, home: &Path) -> Config {
    load_config(text, home).unwrap()
}

pub fn empty_config(home: &Path) -> Config {
    config(r#"{ "rules": [] }"#, home)
}

/// Makes `link` a symlink to `target`. On Windows this needs Developer
/// Mode or an administrator.
pub fn symlink(target: impl AsRef<Path>, link: impl AsRef<Path>) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let target = target.as_ref();
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link).unwrap();
        } else {
            std::os::windows::fs::symlink_file(target, link).unwrap();
        }
    }
}
