//! Runs the approved Operations of a Plan as one Session, journaling each
//! one before it touches the disk.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::journal::{lock_sessions, Journal, Record, SessionError};
use crate::planner::{Operation, OperationKind, Plan};
use crate::protection::HOLDING_FOLDER_NAME;
use crate::scanner::{millis, total_size};

/// What happened when a Session ran.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub session_id: String,
    pub done: Vec<Outcome>,
    pub skipped: Vec<Outcome>,
    pub failed: Vec<Outcome>,
}

/// One Operation's outcome, in a Session or an Undo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub operation_id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    pub destination: PathBuf,
    /// Why it was skipped or failed.
    pub reason: Option<String>,
}

/// What an Operation's source was like when planned: if it differs when
/// the Operation runs, or is redone, the Operation is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceState {
    /// Bytes; for a Package, the total of everything inside it.
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub modified: i64,
    pub is_package: bool,
}

/// Runs the Operations of `plan` whose IDs are in `approved`, in Plan
/// order, as a new Session journaled in `journal_dir`. Trashed items go to
/// `<Scan Root>/Files Cleaner Trash/<Session ID>/<original relative path>`.
pub fn run(plan: &Plan, approved: &[String], journal_dir: &Path) -> Result<Summary, SessionError> {
    let _lock = lock_sessions(journal_dir)?;
    let approved: HashSet<&str> = approved.iter().map(String::as_str).collect();
    let (session_id, mut journal) = start_journal(journal_dir)?;
    journal.append(&Record::Session {
        id: session_id.clone(),
        scan_root: plan.root.clone(),
        trash_destination: plan.trash_destination,
        started_at: millis(SystemTime::now()),
    })?;

    let holding = plan.root.join(HOLDING_FOLDER_NAME).join(&session_id);
    let mut summary = Summary { session_id, done: Vec::new(), skipped: Vec::new(), failed: Vec::new() };
    for op in plan.operations.iter().filter(|op| approved.contains(op.id.as_str())) {
        let destination = holding.join(op.relative_path.iter().collect::<PathBuf>());
        journal.append(&Record::Intent {
            operation_id: op.id.clone(),
            kind: op.kind,
            source: op.source.clone(),
            destination: destination.clone(),
            planned: Some(planned(op)),
        })?;
        let mut outcome = Outcome {
            operation_id: op.id.clone(),
            kind: op.kind,
            source: op.source.clone(),
            destination: destination.clone(),
            reason: None,
        };
        let operation_id = op.id.clone();
        if let Err(reason) = unchanged(&op.source, &planned(op), "since the Plan was made") {
            journal.append(&Record::Skipped { operation_id, reason: reason.clone() })?;
            outcome.reason = Some(reason);
            summary.skipped.push(outcome);
        } else if let Err(reason) = move_without_overwriting(&op.source, &destination) {
            journal.append(&Record::Failed { operation_id, reason: reason.clone() })?;
            outcome.reason = Some(reason);
            summary.failed.push(outcome);
        } else {
            journal.append(&Record::Done { operation_id })?;
            summary.done.push(outcome);
        }
    }
    journal.append(&Record::Finished)?;
    Ok(summary)
}

/// A new Session ID (the UTC start time) and its Journal.
fn start_journal(journal_dir: &Path) -> Result<(String, Journal), SessionError> {
    let base = session_id(SystemTime::now());
    for n in 1.. {
        let id = if n == 1 { base.clone() } else { format!("{base}-{n}") };
        match Journal::create(journal_dir, &id) {
            Ok(journal) => return Ok((id, journal)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!()
}

/// `2026-10-14_09-05-03-123Z`: sorts by time, and is a valid file name on
/// every OS.
fn session_id(time: SystemTime) -> String {
    let ms = time.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let (secs, ms) = (ms / 1000, ms % 1000);
    let (days, secs) = (secs / 86_400, secs % 86_400);
    let (year, month, day) = civil_from_days(days as i64);
    let (hour, minute, second) = (secs / 3600, secs / 60 % 60, secs % 60);
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}-{ms:03}Z")
}

/// The (year, month, day) `days` after 1970-01-01, in the proleptic
/// Gregorian calendar (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn planned(op: &Operation) -> SourceState {
    SourceState { size: op.size, modified: op.modified, is_package: op.is_package }
}

/// Fails with the reason if `path` is gone or isn't the item `expected`
/// describes: a different kind, size or modified date. `since` ends the
/// reason ("since the Plan was made").
pub(crate) fn unchanged(path: &Path, expected: &SourceState, since: &str) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err("It no longer exists".into()),
        Err(e) => return Err(format!("It couldn't be read: {e}")),
    };
    let is_package = metadata.is_dir();
    if metadata.is_symlink() || is_package != expected.is_package {
        return Err(format!("It was replaced by something else {since}"));
    }
    let size = if is_package { total_size(path) } else { metadata.len() };
    if size != expected.size {
        return Err(format!("Its size changed {since}"));
    }
    match metadata.modified() {
        Ok(modified) if millis(modified) == expected.modified => Ok(()),
        _ => Err(format!("It was modified {since}")),
    }
}

/// Moves `from` to `to` with a same-disk rename, creating `to`'s folders.
/// Never replaces anything already at `to`, and never copies.
pub(crate) fn move_without_overwriting(from: &Path, to: &Path) -> Result<(), String> {
    if fs::symlink_metadata(to).is_ok() {
        return Err(format!("{} already exists", to.display()));
    }
    let moved = to.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| fs::rename(from, to));
    moved.map_err(|e| match e.kind() {
        io::ErrorKind::CrossesDevices => format!("{} is on a different disk from {}", from.display(), to.display()),
        _ => format!("Couldn't move it to {}: {e}", to.display()),
    })
}

