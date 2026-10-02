#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use files_cleaner_core::{load_config, Config};
use tempfile::TempDir;

/// A temp folder for a test to scan.
///
/// It lives under the crate folder rather than the system temp dir: macOS
/// puts that under `/private`, and `target/` sits next to a `Cargo.toml`,
/// and both are Protected Paths the scanner would refuse.
pub fn scan_root() -> TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(base)
        .unwrap()
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
