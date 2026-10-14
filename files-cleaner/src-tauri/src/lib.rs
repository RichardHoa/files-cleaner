//! Thin Tauri commands over the core library (ADR-0003): they find the
//! config file and home folder, call the core, and serialise the result.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use files_cleaner_core::{
    self as core, Config, Plan, RecentScanRoot, RedoSummary, ScanProgress, ScanResult, SessionDetails, SessionPage,
    Summary, UndoSummary, EXAMPLE_CONFIG,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

const CONFIG_FILE_NAME: &str = "config.json";
/// The Journal folder, inside the app data folder.
const SESSIONS_FOLDER_NAME: &str = "sessions";
/// The Recent Scan Roots, inside the app data folder.
const RECENT_FILE_NAME: &str = "recent-scan-roots.json";

/// The latest scan and Plan, kept here so the UI doesn't send them back.
#[derive(Default)]
struct Latest {
    scan: Mutex<Option<ScanResult>>,
    plan: Mutex<Option<Plan>>,
}

/// An error the UI can show: what went wrong and, for a config error,
/// where in the file.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    message: String,
    /// Set when the problem is in the config file.
    config_path: Option<PathBuf>,
    line: Option<usize>,
    column: Option<usize>,
}

impl CommandError {
    fn new(message: impl ToString) -> Self {
        CommandError { message: message.to_string(), config_path: None, line: None, column: None }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadedConfig {
    path: PathBuf,
    config: Config,
}

/// `name` inside the app data folder.
fn app_data_path(app: &AppHandle, name: &str) -> Result<PathBuf, CommandError> {
    Ok(app.path().app_data_dir().map_err(CommandError::new)?.join(name))
}

fn config_path(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app_data_path(app, CONFIG_FILE_NAME)
}

/// The config file's text, writing the example config first if there is
/// no file yet.
fn read_or_create(path: &Path) -> Result<String, CommandError> {
    if !path.exists() {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(CommandError::new)?;
        }
        fs::write(path, EXAMPLE_CONFIG).map_err(CommandError::new)?;
    }
    fs::read_to_string(path).map_err(CommandError::new)
}

/// Reads the config file afresh, so edits apply without a restart.
fn read_config(app: &AppHandle) -> Result<LoadedConfig, CommandError> {
    let path = config_path(app)?;
    let text = read_or_create(&path)?;
    let home = app.path().home_dir().map_err(CommandError::new)?;
    match core::load_config(&text, &home) {
        Ok(config) => Ok(LoadedConfig { path, config }),
        Err(e) => Err(CommandError {
            message: e.message,
            config_path: Some(path),
            line: e.line,
            column: e.column,
        }),
    }
}

#[tauri::command]
fn load_config(app: AppHandle) -> Result<LoadedConfig, CommandError> {
    read_config(&app)
}

#[tauri::command]
fn open_config(app: AppHandle) -> Result<(), CommandError> {
    let path = config_path(&app)?;
    read_or_create(&path)?;
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(CommandError::new)
}

/// Why the user should think twice before scanning `scan_root`, if they
/// should. Any folder can still be scanned.
#[tauri::command]
fn scan_root_warning(app: AppHandle, scan_root: PathBuf) -> Result<Option<String>, CommandError> {
    let loaded = read_config(&app)?;
    Ok(core::scan_root_warning(&scan_root, &loaded.config))
}

/// Scans off the UI thread, sending progress to `on_progress` as it goes.
/// The result is kept for `build_plan`. The UI calls this only once any
/// protected-path warning is accepted, so `scan_root` becomes a Recent
/// Scan Root here.
#[tauri::command]
async fn scan(
    app: AppHandle,
    latest: State<'_, Latest>,
    scan_root: PathBuf,
    on_progress: Channel<ScanProgress>,
) -> Result<ScanResult, CommandError> {
    let loaded = read_config(&app)?;
    // A list that can't be saved only loses the shortcut; the scan goes on.
    let recorded = recent_store(&app)
        .and_then(|store| core::record_recent_scan_root(&store, &scan_root).map_err(CommandError::new));
    if let Err(e) = recorded {
        eprintln!("Couldn't save the Recent Scan Roots: {}", e.message);
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        // A closed window can't receive progress; the scan carries on.
        core::scan_with_progress(&scan_root, &loaded.config, |progress| {
            let _ = on_progress.send(progress);
        })
    })
        .await
        .map_err(CommandError::new)?
        .map_err(CommandError::new)?;
    *latest.scan.lock().unwrap() = Some(result.clone());
    Ok(result)
}

/// Plans the latest scan with the config as it is now, off the UI thread
/// (finding duplicates reads files). The Plan is kept for `run`.
#[tauri::command]
async fn build_plan(app: AppHandle, latest: State<'_, Latest>) -> Result<Plan, CommandError> {
    let loaded = read_config(&app)?;
    let scan = latest.scan.lock().unwrap().clone().ok_or_else(|| CommandError::new("Scan a folder first"))?;
    let plan = tauri::async_runtime::spawn_blocking(move || core::build_plan(&scan, &loaded.config))
        .await
        .map_err(CommandError::new)?;
    *latest.plan.lock().unwrap() = Some(plan.clone());
    Ok(plan)
}

/// Where the Journals live: a folder in the app data folder.
fn journal_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app_data_path(app, SESSIONS_FOLDER_NAME)
}

/// Runs the approved Operations of the latest Plan as a new Session. The
/// Plan is kept, so a run refused as Busy can be retried; running it again
/// after it ran starts a new Session whose Operations are all skipped.
#[tauri::command]
async fn run_plan(app: AppHandle, latest: State<'_, Latest>, approved: Vec<String>) -> Result<Summary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    let plan = latest.plan.lock().unwrap().clone().ok_or_else(|| CommandError::new("Build a Plan first"))?;
    tauri::async_runtime::spawn_blocking(move || core::run(&plan, &approved, &journal_dir))
        .await
        .map_err(CommandError::new)?
        .map_err(CommandError::new)
}

/// Undoes a past Session off the UI thread.
#[tauri::command]
async fn undo(app: AppHandle, session_id: String) -> Result<UndoSummary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || core::undo(&session_id, &journal_dir))
        .await
        .map_err(CommandError::new)?
        .map_err(CommandError::new)
}

/// Undoes only the chosen Operations of a past Session.
#[tauri::command]
async fn undo_operations(
    app: AppHandle,
    session_id: String,
    operation_ids: Vec<String>,
) -> Result<UndoSummary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || core::undo_operations(&session_id, &operation_ids, &journal_dir))
        .await
        .map_err(CommandError::new)?
        .map_err(CommandError::new)
}

/// Carries out again the chosen undone Operations of a past Session.
#[tauri::command]
async fn redo_operations(
    app: AppHandle,
    session_id: String,
    operation_ids: Vec<String>,
) -> Result<RedoSummary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || core::redo_operations(&session_id, &operation_ids, &journal_dir))
        .await
        .map_err(CommandError::new)?
        .map_err(CommandError::new)
}

/// A past Session and each of its Operations, for the Session screen.
#[tauri::command]
fn session_details(app: AppHandle, session_id: String) -> Result<SessionDetails, CommandError> {
    core::session_details(&session_id, &journal_dir(&app)?).map_err(CommandError::new)
}

/// A page of past Sessions, newest first, for Home and History.
#[tauri::command]
fn list_sessions(app: AppHandle, offset: usize, limit: usize) -> Result<SessionPage, CommandError> {
    core::list_sessions(&journal_dir(&app)?, offset, limit).map_err(CommandError::new)
}

/// Where the Recent Scan Roots live: a file in the app data folder.
fn recent_store(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app_data_path(app, RECENT_FILE_NAME)
}

/// The Recent Scan Roots, newest first, for the Home tiles.
#[tauri::command]
fn recent_scan_roots(app: AppHandle) -> Result<Vec<RecentScanRoot>, CommandError> {
    Ok(core::recent_scan_roots(&recent_store(&app)?))
}

/// Takes a folder off the Recent Scan Roots, from the × on its tile.
#[tauri::command]
fn remove_recent_scan_root(app: AppHandle, scan_root: PathBuf) -> Result<(), CommandError> {
    core::remove_recent_scan_root(&recent_store(&app)?, &scan_root).map_err(CommandError::new)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Latest::default())
        .invoke_handler(tauri::generate_handler![
            load_config,
            open_config,
            scan_root_warning,
            scan,
            build_plan,
            run_plan,
            undo,
            undo_operations,
            redo_operations,
            session_details,
            list_sessions,
            recent_scan_roots,
            remove_recent_scan_root
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
