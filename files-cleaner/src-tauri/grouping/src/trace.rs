//! For tuning the grouping. Holds file contents, so the app turns it on in
//! debug builds only.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static FOLDER: OnceLock<PathBuf> = OnceLock::new();

pub fn trace_to(folder: impl Into<PathBuf>) {
    let _ = FOLDER.set(folder.into());
}

/// So each analysis's traces stand alone.
pub fn clear_trace() {
    if let Some(folder) = FOLDER.get() {
        let _ = fs::remove_dir_all(folder);
    }
}

/// Lets callers skip building the text.
pub(crate) fn enabled() -> bool {
    FOLDER.get().is_some()
}

pub(crate) const CARDS: &str = "cards";
pub(crate) const PLACING: &str = "placing";
pub(crate) const SIMILARITY: &str = "similarity";
pub(crate) const TIMINGS: &str = "timings";

fn path(kind: &str, name: &str) -> Option<PathBuf> {
    let folder = FOLDER.get()?.join(kind);
    fs::create_dir_all(&folder).ok()?;
    let name: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') {
                '_'
            } else {
                c
            }
        })
        .collect();
    Some(folder.join(name))
}

/// Only for tuning, so failures are ignored.
pub(crate) fn write(kind: &str, name: &str, text: &str) {
    if let Some(path) = path(kind, name) {
        let _ = fs::write(path, text);
    }
}

pub(crate) fn time(text: &str) {
    use std::io::Write;
    if let Some(path) = path(TIMINGS, "timings.log") {
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut f| f.write_all(text.as_bytes()));
    }
}

pub(crate) fn secs(d: std::time::Duration) -> String {
    format!("{:.2} s", d.as_secs_f64())
}
