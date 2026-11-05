//! The Windows System Trash: `IFileOperation` with `FOFX_RECYCLEONDELETE`,
//! and a progress sink to learn where each item landed.
//!
//! A trash must never become a permanent delete (ADR-0001), and this operation
//! deletes outright when the Recycle Bin can't take an item. So the drive is
//! checked first, and the sink vetoes any delete not flagged as a recycle. If
//! Windows leaves that flag off a real recycle, items are refused and stay in
//! place: the safe way to be wrong.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
        Some(Err(error)) => Err(format!("The Recycle Bin gave no file path for it: {error}")),
        None => Err("The Recycle Bin didn't say where it landed".to_string()),
    }
}

pub fn put_back(landed: &Path, to: &Path) -> Result<(), String> {
    move_without_overwriting(landed, to)
}

pub fn forget(_landed: &Path) {}
