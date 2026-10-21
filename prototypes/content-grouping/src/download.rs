//! One-time downloads (the model, PDFium) with the system `curl`, shipped with
//! macOS, Windows 10+ and most Linux distributions. Nothing else in the
//! program touches the network.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The user cache folder this prototype keeps its downloads in.
pub fn cache_root() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("files-cleaner-content-grouping"))
}

/// Downloads `url` to `dest` through `dest.part`, which an interrupted run
/// resumes. `dest` only appears once the download is complete.
pub fn fetch(url: &str, dest: &Path) -> Result<(), String> {
    eprintln!("Downloading {url}");
    let mut partial = dest.as_os_str().to_owned();
    partial.push(".part");
    let partial = PathBuf::from(partial);
    let status = Command::new("curl")
        .args(["--fail", "--location", "--progress-bar", "--continue-at", "-", "--output"])
        .arg(&partial)
        .arg(url)
        .status()
        .map_err(|e| format!("can't run curl to download {url} ({e}); download it to {} yourself", dest.display()))?;
    if !status.success() {
        return Err(format!("curl failed downloading {url}"));
    }
    std::fs::rename(&partial, dest).map_err(|e| e.to_string())
}
