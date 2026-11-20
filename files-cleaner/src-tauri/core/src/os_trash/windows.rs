//! The Windows System Trash: `IFileOperation` with `FOFX_RECYCLEONDELETE`,
//! and a progress sink to learn where each item landed.
//!
//! A trash must never become a permanent delete, and this operation
//! deletes outright when the Recycle Bin can't take an item. So the drive is
//! checked first, and the sink vetoes any delete not flagged as a recycle. If
//! Windows leaves that flag off a real recycle, items are refused and stay in
//! place: the safe way to be wrong.

use std::ffi::{c_void, OsString};
use std::fs;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::E_ABORT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IFileOperationProgressSink, IFileOperationProgressSink_Impl,
    IShellItem, SHCreateItemFromParsingName, SHQueryRecycleBinW, FOFX_RECYCLEONDELETE,
    FOF_ALLOWUNDO, FOF_NO_UI, SHQUERYRBINFO, SIGDN_FILESYSPATH, TSF_DELETE_RECYCLE_IF_POSSIBLE,
};
use windows_core::{implement, Ref, Result as WinResult, HRESULT, PCWSTR};

use crate::executor::move_without_overwriting;

pub const PLACE: &str = "Recycle Bin";

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain([0]).collect()
}

pub fn blocked(folder: &Path) -> Option<String> {
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    let path = wide(folder);
    // SAFETY: `path` is NUL-terminated and `info` has its cbSize set, both
    // living for the whole call.
    let queried = unsafe { SHQueryRecycleBinW(PCWSTR(path.as_ptr()), &mut info) };
    queried
        .err()
        .map(|_| "This drive has no Recycle Bin".to_string())
}

#[derive(Default)]
struct Observed {
    vetoed: bool,
    landing: Option<Result<PathBuf, String>>,
}

#[implement(IFileOperationProgressSink)]
struct Sink {
    observed: Arc<Mutex<Observed>>,
}

fn filesystem_path(item: &IShellItem) -> Result<PathBuf, String> {
    // SAFETY: on success GetDisplayName returns a valid string this function
    // owns, which is freed once and not used after.
    unsafe {
        let name = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|e| e.to_string())?;
        let path = name
            .to_string()
            .map(PathBuf::from)
            .map_err(|e| e.to_string());
        CoTaskMemFree(Some(name.as_ptr() as *const c_void));
        path
    }
}

#[allow(non_snake_case)]
impl IFileOperationProgressSink_Impl for Sink_Impl {
    fn PreDeleteItem(&self, dwflags: u32, _item: Ref<IShellItem>) -> WinResult<()> {
        if dwflags & TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32 == 0 {
            self.observed
                .lock()
                .expect("only this thread uses it")
                .vetoed = true;
            return Err(E_ABORT.into());
        }
        Ok(())
    }

    fn PostDeleteItem(
        &self,
        _dwflags: u32,
        _item: Ref<IShellItem>,
        _hrdelete: HRESULT,
        newly_created: Ref<IShellItem>,
    ) -> WinResult<()> {
        self.observed
            .lock()
            .expect("only this thread uses it")
            .landing = newly_created.as_ref().map(filesystem_path);
        Ok(())
    }

    fn StartOperations(&self) -> WinResult<()> {
        Ok(())
    }
    fn FinishOperations(&self, _hr: HRESULT) -> WinResult<()> {
        Ok(())
    }
    fn PreRenameItem(&self, _f: u32, _i: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PostMoveItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreNewItem(&self, _f: u32, _d: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        _f: u32,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _t: &PCWSTR,
        _a: u32,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _total: u32, _done: u32) -> WinResult<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> WinResult<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> WinResult<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> WinResult<()> {
        Ok(())
    }
}

fn perform_delete(path: &Path, sink: &IFileOperationProgressSink) -> WinResult<()> {
    let path = wide(path);
    // SAFETY: COM is initialized on this thread before any COM call, and
    // uninitialized only after `delete_with` has dropped its COM objects.
    // `path` is NUL-terminated and outlives its use.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let result = delete_with(PCWSTR(path.as_ptr()), sink);
        CoUninitialize();
        result
    }
}

/// # Safety
/// COM must be initialized on this thread, and `path` valid for the call.
unsafe fn delete_with(path: PCWSTR, sink: &IFileOperationProgressSink) -> WinResult<()> {
    let operation: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
    operation.SetOperationFlags(FOF_NO_UI | FOF_ALLOWUNDO | FOFX_RECYCLEONDELETE)?;
    let cookie = operation.Advise(sink)?;
    let item: IShellItem = SHCreateItemFromParsingName(path, None)?;
    operation.DeleteItem(&item, None)?;
    let performed = operation.PerformOperations();
    operation.Unadvise(cookie)?;
    performed
}

pub fn trash(path: &Path) -> Result<PathBuf, String> {
    if let Some(reason) = path.parent().and_then(blocked) {
        return Err(reason);
    }
    // A second early: a FILETIME is finer than some disks keep.
    let started = SystemTime::now() - Duration::from_secs(1);
    let observed = Arc::new(Mutex::new(Observed::default()));
    let sink: IFileOperationProgressSink = Sink {
        observed: observed.clone(),
    }
    .into();
    let performed = perform_delete(path, &sink);

    let observed = observed.lock().expect("only this thread uses it");
    if observed.vetoed {
        return Err(
            "The Recycle Bin can't take it, and the app never deletes for good, so it was left in place"
                .to_string(),
        );
    }
    performed.map_err(|e| format!("The Recycle Bin refused it: {e}"))?;
    match &observed.landing {
        Some(Ok(landing)) => Ok(landing.clone()),
        // It may have left all the same: its record in the Recycle Bin says where to.
        _ if fs::symlink_metadata(path).is_err() => {
            find_landing(path, started).ok_or_else(|| match &observed.landing {
                Some(Err(error)) => format!("The Recycle Bin gave no file path for it: {error}"),
                _ => "The Recycle Bin didn't say where it landed".to_string(),
            })
        }
        Some(Err(error)) => Err(format!("The Recycle Bin gave no file path for it: {error}")),
        None => Err("The Recycle Bin didn't say where it landed".to_string()),
    }
}

/// Seconds from 1601, where a FILETIME starts, to 1970.
const FILETIME_TO_UNIX_SECS: u64 = 11_644_473_600;

/// `time` as a FILETIME: 100 ns steps since 1601.
fn filetime(time: SystemTime) -> u64 {
    let since_1970 = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    (since_1970.as_secs() + FILETIME_TO_UNIX_SECS) * 10_000_000
        + u64::from(since_1970.subsec_nanos()) / 100
}

/// What a `$I` file in the Recycle Bin records of its `$R` item: where it
/// was, and when it was deleted, as a FILETIME. Version 1 (Vista to 8)
/// keeps the path in 260 characters; version 2 gives its length first.
fn parse_deleted_record(bytes: &[u8]) -> Option<(PathBuf, u64)> {
    let u64_at = |at: usize| Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?));
    let deleted = u64_at(16)?;
    let (start, chars) = match u64_at(0)? {
        1 => (24, 260),
        2 => (
            28,
            u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?) as usize,
        ),
        _ => return None,
    };
    let units: Vec<u16> = bytes
        .get(start..start + chars * 2)?
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&unit| unit != 0)
        .collect();
    Some((PathBuf::from(OsString::from_wide(&units)), deleted))
}

/// Where the Recycle Bin put `source`, when Windows didn't say: the `$R`
/// item whose `$I` record names `source`, deleted no earlier than `since`
/// (so never an older item from the same path), the latest if several.
/// Only the user's own folder in `$Recycle.Bin` can be read.
fn find_landing(source: &Path, since: SystemTime) -> Option<PathBuf> {
    let drive: PathBuf = source
        .components()
        .take_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        .collect();
    let since = filetime(since);
    let wanted = source.to_string_lossy().to_lowercase();
    let users = fs::read_dir(drive.join("$Recycle.Bin")).ok()?;
    let records = users
        .flatten()
        .filter_map(|user| fs::read_dir(user.path()).ok())
        .flat_map(|entries| entries.flatten());
    records
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rest = name.strip_prefix("$I")?;
            let (was, deleted) = parse_deleted_record(&fs::read(entry.path()).ok()?)?;
            let matches = was.to_string_lossy().to_lowercase() == wanted && deleted >= since;
            matches.then(|| (deleted, entry.path().with_file_name(format!("$R{rest}"))))
        })
        .filter(|(_, landing)| fs::symlink_metadata(landing).is_ok())
        .max_by_key(|(deleted, _)| *deleted)
        .map(|(_, landing)| landing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_where_a_version_2_record_says_its_item_was() {
        let path: Vec<u16> = r"C:\Users\me\copy.txt".encode_utf16().chain([0]).collect();
        let mut record = Vec::new();
        record.extend(2u64.to_le_bytes());
        record.extend(5u64.to_le_bytes());
        record.extend(133_000_000_000_000_000u64.to_le_bytes());
        record.extend((path.len() as u32).to_le_bytes());
        record.extend(path.iter().flat_map(|unit| unit.to_le_bytes()));

        assert_eq!(
            parse_deleted_record(&record),
            Some((
                PathBuf::from(r"C:\Users\me\copy.txt"),
                133_000_000_000_000_000
            ))
        );
    }
}

pub fn put_back(landed: &Path, to: &Path) -> Result<(), String> {
    move_without_overwriting(landed, to)
}

pub fn forget(_landed: &Path) {}
