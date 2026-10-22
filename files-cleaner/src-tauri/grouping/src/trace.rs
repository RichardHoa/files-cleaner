//! What the grouping did, for tuning it: each prompt the LLM saw with its
//! reply, and how similar each Suggested Folder is to the files outside it,
//! written to the folder `trace_to` names. Nothing is written until it's
//! called; the app calls it in debug builds only.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// Writes traces into `folder` from now on; only the first call counts.
pub fn trace_to(folder: impl Into<PathBuf>) {
    let _ = FOLDER.set(folder.into());
}

/// Removes everything written so far, so the next analysis's traces stand
/// alone.
pub fn clear_trace() {
    if let Some(folder) = FOLDER.get() {
        let _ = fs::remove_dir_all(folder);
    }
}

/// Whether anything is written, so callers can skip building the text.
pub(crate) fn enabled() -> bool {
    FOLDER.get().is_some()
}

/// Writes `text` to `name` in the trace folder. A trace that can't be
/// written is only missing, so failures are ignored.
pub(crate) fn write(name: &str, text: &str) {
    let Some(folder) = FOLDER.get() else { return };
    let name: String = name.chars().map(|c| if matches!(c, '/' | '\\' | ':') { '_' } else { c }).collect();
    let _ = fs::create_dir_all(folder).and_then(|()| fs::write(Path::new(folder).join(name), text));
}

/// Where the time goes, file by file, appended to `_timings.log`.
pub(crate) const TIMINGS: &str = "_timings.log";

/// Adds `text` to the end of `name` in the trace folder.
pub(crate) fn append(name: &str, text: &str) {
    use std::io::Write;
    let Some(folder) = FOLDER.get() else { return };
    let _ = fs::create_dir_all(folder).and_then(|()| {
        fs::OpenOptions::new().create(true).append(true).open(Path::new(folder).join(name))?.write_all(text.as_bytes())
    });
}

/// `d` in seconds, as timings are written.
pub(crate) fn secs(d: std::time::Duration) -> String {
    format!("{:.2} s", d.as_secs_f64())
}
