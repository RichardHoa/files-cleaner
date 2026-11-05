//! Draft macOS backend: NSFileManager, as ADR 0001 says.

use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2_foundation::{NSError, NSFileManager, NSSearchPathDirectory, NSSearchPathDomainMask, NSString, NSURL};

use crate::{Preflight, Refusal, Verdict};

pub const PLACE: &str = "Trash";
pub const OS_NOTE: &str = "run it with Full Disk Access off for this terminal";
/// macOS keeps no separate per-item record for this test to check.
pub const KEEPS_RECORD: bool = false;
/// Foundation's `NSFeatureUnsupportedError`, reported for a disk with no Trash.
const FEATURE_UNSUPPORTED: isize = 3328;

fn url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

fn describe(error: &NSError) -> String {
    format!("{} (code {})", error.localizedDescription(), error.code())
}

/// Asks where this item's Trash is, without creating one. Information only:
/// the call in `trash` is what decides.
pub fn preflight(path: &Path) -> Preflight {
    let found = NSFileManager::defaultManager().URLForDirectory_inDomain_appropriateForURL_create_error(
        NSSearchPathDirectory::TrashDirectory,
        NSSearchPathDomainMask::UserDomainMask,
        Some(&url(path)),
        false,
    );
    match found {
        Ok(trash) => Preflight {
            can_trash: true,
            detail: format!("NSFileManager names {:?}", trash.path().map(|p| p.to_string())),
        },
        Err(error) => Preflight { can_trash: false, detail: describe(&error) },
    }
}

pub fn trash(path: &Path, _refuse_permanent: bool) -> Result<PathBuf, Refusal> {
    let mut landed = None;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url(path), Some(&mut landed))
        .map_err(|error| {
            if error.code() == FEATURE_UNSUPPORTED {
                Refusal::NoTrash(describe(&error))
            } else {
                Refusal::Other(describe(&error))
            }
        })?;
    landed
        .and_then(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
        .ok_or_else(|| "the OS trashed it but didn't say where it landed".into())
}

pub fn restore(landing: &Path, to: &Path) -> Result<(), String> {
    crate::move_without_overwriting(landing, to)
}

pub fn leftover(_landing: &Path) -> Option<PathBuf> {
    None
}

pub fn bin_count(_path: &Path) -> Option<i64> {
    None
}

/// Listing `~/.Trash` is what Full Disk Access unlocks, so it tells whether
/// this run says anything about a user who hasn't granted it.
pub fn access_check(restored: usize, errors: &[&str]) -> Option<(Verdict, String)> {
    let home = PathBuf::from(env::var_os("HOME")?);
    let listing = fs::read_dir(home.join(".Trash"));
    let locked_out = matches!(&listing, Err(e) if e.kind() == ErrorKind::PermissionDenied);
    let denied = errors.iter().any(|e| e.contains("not permitted") || e.contains("denied"));
    Some(if !locked_out {
        (
            Verdict::NotAnswered,
            "this terminal isn't locked out of ~/.Trash, so it may have Full Disk Access. Turn that off and run again".to_string(),
        )
    } else if denied {
        (Verdict::Fail, format!("the OS refused to move items back: {errors:?}. Full Disk Access is needed"))
    } else if restored > 0 {
        (Verdict::Pass, format!("{restored} item(s) moved back from a terminal locked out of ~/.Trash"))
    } else {
        (Verdict::NotAnswered, "no item came back, for reasons other than permission".to_string())
    })
}
