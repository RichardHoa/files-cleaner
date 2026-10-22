//! What the grouping did, for tuning it: each prompt the LLM saw with its
//! reply, how similar each Suggested Folder is to the files outside it, and
//! where the time went, each in its own subfolder of the folder `trace_to`
//! names. Nothing is written until it's
//! called; the app calls it in debug builds only.

use std::fs;
use std::path::PathBuf;
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

/// The card prompt and reply for each file.
pub(crate) const CARDS: &str = "cards";
/// The placing prompt and reply for each file left out of every folder.
pub(crate) const PLACING: &str = "placing";
/// How similar the files and folders are, in `similarity.txt`.
pub(crate) const SIMILARITY: &str = "similarity";
/// Where the time goes, file by file, appended to `timings.log`.
pub(crate) const TIMINGS: &str = "timings";

/// Where `name` goes in subfolder `kind` of the trace folder, which is
/// made if need be; `None` when nothing is written.
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

/// Writes `text` to `name` in subfolder `kind` of the trace folder. A trace
/// that can't be written is only missing, so failures are ignored.
pub(crate) fn write(kind: &str, name: &str, text: &str) {
    if let Some(path) = path(kind, name) {
        let _ = fs::write(path, text);
    }
}

/// Adds `text` to the end of `timings.log`.
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

/// `d` in seconds, as timings are written.
pub(crate) fn secs(d: std::time::Duration) -> String {
    format!("{:.2} s", d.as_secs_f64())
}
