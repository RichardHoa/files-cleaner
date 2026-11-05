//! Draft Windows backend: `IFileOperation` with `FOFX_RECYCLEONDELETE`, and a
//! progress sink to learn where each item landed.
//!
//! Microsoft documents `TSF_DELETE_RECYCLE_IF_POSSIBLE` only as "recycle if
//! possible", so whether it tells a recycle from a permanent delete is one of
//! the things this finds out. The flags are printed on every delete. Only the
//! no-Trash case refuses on them, so the ordinary cases still answer their own
//! questions if the flag turns out to mean something else. If a real recycle
//! ever arrives without the flag, the flag proves nothing, so the no-Trash case
//! doesn't refuse on it either: a refusal there would be a false PASS.
//!
//! So this draft can permanently delete its own test file, which is what the
//! test has to find out. Ticket 02 must not inherit that: ADR 0001 says a trash
//! never becomes a permanent delete.

use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::E_ABORT;
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem,
    SHCreateItemFromParsingName, SHQueryRecycleBinW, FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NO_UI,
    SHQUERYRBINFO, SIGDN_FILESYSPATH, TSF_DELETE_RECYCLE_IF_POSSIBLE,
};
use windows_core::{implement, Ref, Result as WinResult, HRESULT, PCWSTR};

use crate::{Preflight, Refusal, Verdict};

pub const PLACE: &str = "Recycle Bin";
pub const OS_NOTE: &str = "paste the flags printed above too";
pub const KEEPS_RECORD: bool = true;

/// Set once a real recycle has arrived without the flag.
static FLAG_UNRELIABLE: AtomicBool = AtomicBool::new(false);

fn says_recycle(flags: u32) -> bool {
    flags & TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32 != 0
}

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain([0]).collect()
}

fn query_bin(path: &Path) -> WinResult<SHQUERYRBINFO> {
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    let path = wide(path.as_os_str());
    // SAFETY: `path` is NUL-terminated and `info` has its cbSize set, both
    // living for the whole call.
    unsafe { SHQueryRecycleBinW(PCWSTR(path.as_ptr()), &mut info)? };
    Ok(info)
}

fn drive_type(path: &Path) -> &'static str {
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return "unknown";
    };
    let mut root = prefix.as_os_str().to_os_string();
    root.push("\\");
    let root = wide(&root);
    // SAFETY: `root` is NUL-terminated and lives for the whole call.
    match unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } {
        2 => "removable",
        3 => "fixed",
        4 => "network",
        5 => "optical",
        6 => "RAM disk",
        _ => "unknown",
    }
}

pub fn preflight(path: &Path) -> Preflight {
    let drive = drive_type(path);
    match query_bin(path) {
        Ok(info) => Preflight {
            can_trash: true,
            detail: format!("{drive} drive; SHQueryRecycleBin succeeded, {} items", info.i64NumItems),
        },
        Err(error) => Preflight {
            can_trash: false,
            detail: format!("{drive} drive; SHQueryRecycleBin failed: {:#010x}", error.code().0),
        },
    }
}

#[derive(Default)]
struct Observed {
    pre_flags: Option<u32>,
    vetoed: bool,
    hr_delete: Option<HRESULT>,
    landing: Option<Result<PathBuf, String>>,
}

#[implement(IFileOperationProgressSink)]
struct Sink {
    refuse_permanent: bool,
    observed: Arc<Mutex<Observed>>,
}

fn filesystem_path(item: &IShellItem) -> Result<PathBuf, String> {
    // SAFETY: on success GetDisplayName returns a valid string this function
    // owns, which is freed once and not used after.
    unsafe {
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|e| e.to_string())?;
        let path = name.to_string().map(PathBuf::from).map_err(|e| e.to_string());
        CoTaskMemFree(Some(name.as_ptr() as *const _));
        path
    }
}

#[allow(non_snake_case)]
impl IFileOperationProgressSink_Impl for Sink_Impl {
    fn PreDeleteItem(&self, dwflags: u32, _item: Ref<IShellItem>) -> WinResult<()> {
        let mut observed = self.observed.lock().expect("only this thread uses it");
        observed.pre_flags = Some(dwflags);
        if self.refuse_permanent && !says_recycle(dwflags) {
            observed.vetoed = true;
            return Err(E_ABORT.into());
        }
        Ok(())
    }

    fn PostDeleteItem(&self, _dwflags: u32, _item: Ref<IShellItem>, hrdelete: HRESULT, newly_created: Ref<IShellItem>) -> WinResult<()> {
        let mut observed = self.observed.lock().expect("only this thread uses it");
        observed.hr_delete = Some(hrdelete);
        observed.landing = newly_created.as_ref().map(filesystem_path);
        Ok(())
    }

    fn StartOperations(&self) -> WinResult<()> { Ok(()) }
    fn FinishOperations(&self, _hr: HRESULT) -> WinResult<()> { Ok(()) }
    fn PreRenameItem(&self, _f: u32, _i: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> { Ok(()) }
    fn PostRenameItem(&self, _f: u32, _i: Ref<IShellItem>, _n: &PCWSTR, _hr: HRESULT, _c: Ref<IShellItem>) -> WinResult<()> { Ok(()) }
    fn PreMoveItem(&self, _f: u32, _i: Ref<IShellItem>, _d: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> { Ok(()) }
    fn PostMoveItem(&self, _f: u32, _i: Ref<IShellItem>, _d: Ref<IShellItem>, _n: &PCWSTR, _hr: HRESULT, _c: Ref<IShellItem>) -> WinResult<()> { Ok(()) }
    fn PreCopyItem(&self, _f: u32, _i: Ref<IShellItem>, _d: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> { Ok(()) }
    fn PostCopyItem(&self, _f: u32, _i: Ref<IShellItem>, _d: Ref<IShellItem>, _n: &PCWSTR, _hr: HRESULT, _c: Ref<IShellItem>) -> WinResult<()> { Ok(()) }
    fn PreNewItem(&self, _f: u32, _d: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> { Ok(()) }
    fn PostNewItem(&self, _f: u32, _d: Ref<IShellItem>, _n: &PCWSTR, _t: &PCWSTR, _a: u32, _hr: HRESULT, _c: Ref<IShellItem>) -> WinResult<()> { Ok(()) }
    fn UpdateProgress(&self, _total: u32, _done: u32) -> WinResult<()> { Ok(()) }
    fn ResetTimer(&self) -> WinResult<()> { Ok(()) }
    fn PauseTimer(&self) -> WinResult<()> { Ok(()) }
    fn ResumeTimer(&self) -> WinResult<()> { Ok(()) }
}

fn perform_delete(path: &Path, sink: &IFileOperationProgressSink) -> WinResult<bool> {
    let path = wide(path.as_os_str());
    // SAFETY: COM is initialized on this thread before any COM call, `path` is
    // NUL-terminated and outlives its use, and the sink is unadvised before
    // the operation is dropped.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let operation: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
        operation.SetOperationFlags(FOF_NO_UI | FOF_ALLOWUNDO | FOFX_RECYCLEONDELETE)?;
        let cookie = operation.Advise(sink)?;
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None)?;
        operation.DeleteItem(&item, None)?;
        let performed = operation.PerformOperations();
        let aborted = operation.GetAnyOperationsAborted();
        operation.Unadvise(cookie)?;
        if let Err(error) = performed {
            println!("       PerformOperations failed: {:#010x}", error.code().0);
        }
        Ok(aborted?.as_bool())
    }
}

pub fn trash(path: &Path, refuse_permanent: bool) -> Result<PathBuf, Refusal> {
    if refuse_permanent {
        let preflight = preflight(path);
        if !preflight.can_trash {
            return Err(Refusal::NoTrash(format!("refused before touching it: {}", preflight.detail)));
        }
    }
    let flag_unreliable = FLAG_UNRELIABLE.load(Ordering::Relaxed);
    if refuse_permanent && flag_unreliable {
        println!("       recycles here arrive without the flag, so the sink can't refuse on it: the OS decides");
    }
    let observed = Arc::new(Mutex::new(Observed::default()));
    let sink: IFileOperationProgressSink =
        Sink { refuse_permanent: refuse_permanent && !flag_unreliable, observed: observed.clone() }.into();
    let aborted = perform_delete(path, &sink).map_err(|e| e.to_string())?;

    let observed = observed.lock().expect("only this thread uses it");
    println!(
        "       sink: PreDeleteItem flags {:?}, PostDeleteItem result {:?}, newly created item: {}, aborted: {aborted}",
        observed.pre_flags.map(|flags| format!("{flags:#x}")),
        observed.hr_delete.map(|hr| format!("{:#010x}", hr.0)),
        observed.landing.is_some(),
    );
    if observed.vetoed {
        return Err(Refusal::NoTrash(format!(
            "the sink refused it: flags {:?} don't say it will be recycled",
            observed.pre_flags,
        )));
    }
    if observed.landing.is_some() && !observed.pre_flags.is_some_and(says_recycle) {
        FLAG_UNRELIABLE.store(true, Ordering::Relaxed);
    }
    match &observed.landing {
        Some(Ok(landing)) => Ok(landing.clone()),
        Some(Err(error)) => Err(format!("the sink got an item with no file path: {error}").into()),
        None => Err("the sink got no Recycle Bin item, which Microsoft documents as a full delete".into()),
    }
}

pub fn restore(landing: &Path, to: &Path) -> Result<(), String> {
    crate::move_without_overwriting(landing, to)
}

/// The `$I…` record beside a `$R…` landing, if it's still there.
pub fn leftover(landing: &Path) -> Option<PathBuf> {
    let name = landing.file_name()?.to_str()?;
    let record = landing.with_file_name(format!("$I{}", name.strip_prefix("$R")?));
    fs::symlink_metadata(&record).is_ok().then_some(record)
}

pub fn bin_count(path: &Path) -> Option<i64> {
    query_bin(path).ok().map(|info| info.i64NumItems)
}

pub fn access_check(_restored: usize, _errors: &[&str]) -> Option<(Verdict, String)> {
    None
}
