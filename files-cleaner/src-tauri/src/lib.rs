//! Thin on purpose: the logic lives in the core library (ADR-0003).

use std::collections::HashSet;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod downloads;
mod menu;
mod models;

use files_cleaner_core::write_atomically;
use files_cleaner_core::{
    self as core, Config, DuplicateChoice, DuplicateGroup, DuplicateProgress, Plan, RecentScanRoot,
    RunProgress, ScanProgress, ScanResult, SessionDetails, SessionPage, SettingChange,
    SuggestedFolderChoice, Summary, SystemTrash, EXAMPLE_CONFIG,
};
use files_cleaner_grouping::{self as grouping, CardStore, Grouping, LlamaModels};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use models::GroupingModels;

const CONFIG_FILE_NAME: &str = "config.json";
const SESSIONS_FOLDER_NAME: &str = "sessions";
const RECENT_FILE_NAME: &str = "recent-scan-roots.json";
const MODELS_FOLDER_NAME: &str = "grouping-models";
const ONBOARDING_FILE_NAME: &str = "onboarding.json";
const CARDS_FOLDER_NAME: &str = "grouping-cards";
const READING_PACE_FILE_NAME: &str = "reading-pace.json";
#[cfg(target_os = "macos")]
const PDFIUM_LIBRARY: &str = "pdfium/libpdfium.dylib";
#[cfg(target_os = "windows")]
const PDFIUM_LIBRARY: &str = "pdfium/pdfium.dll";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PDFIUM_LIBRARY: &str = "pdfium/libpdfium.so";
const TESSDATA_FOLDER: &str = "tessdata";

/// Kept here so the UI doesn't send them back. One lock holds them all, so
/// neither outlives its scan.
#[derive(Default)]
struct Latest(Mutex<Option<Scanned>>);

struct Scanned {
    scan: Arc<ScanResult>,
    duplicates: Option<Arc<Vec<DuplicateGroup>>>,
    plan: Option<Plan>,
}

/// Cancelling covers both the scan and the search for duplicates after it.
#[derive(Default)]
struct Scanning(Mutex<ScanFlag>);

#[derive(Default)]
struct ScanFlag {
    id: u64,
    cancel: Arc<AtomicBool>,
    /// A Cancel can reach the app before its scan has started; that scan then
    /// starts cancelled.
    cancelled: u64,
}

/// Like `Scanning`: a Cancel can reach the app before its Run has started.
#[derive(Default)]
struct Running(Mutex<ScanFlag>);

#[derive(Default)]
struct Analysing {
    cancel: Mutex<Arc<AtomicBool>>,
    stop: Mutex<Arc<AtomicBool>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    message: String,
    config_path: Option<PathBuf>,
    line: Option<usize>,
    column: Option<usize>,
}

impl CommandError {
    fn new(message: impl ToString) -> Self {
        CommandError {
            message: message.to_string(),
            config_path: None,
            line: None,
            column: None,
        }
    }
}

async fn blocking<T, E>(
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, CommandError>
where
    T: Send + 'static,
    E: ToString + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(CommandError::new)?
        .map_err(CommandError::new)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadedConfig {
    path: PathBuf,
    config: Config,
}

fn app_data_path(app: &AppHandle, name: &str) -> Result<PathBuf, CommandError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(CommandError::new)?
        .join(name))
}

fn config_path(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app_data_path(app, CONFIG_FILE_NAME)
}

/// Writes the example config on first launch.
fn read_or_create(path: &Path) -> Result<String, CommandError> {
    if !path.exists() {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(CommandError::new)?;
        }
        write_atomically(path, EXAMPLE_CONFIG.as_bytes()).map_err(CommandError::new)?;
    }
    fs::read_to_string(path).map_err(CommandError::new)
}

/// Read afresh, so every command sees the latest saved settings.
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
fn reset_settings(app: AppHandle) -> Result<LoadedConfig, CommandError> {
    let path = config_path(&app)?;
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(CommandError::new)?;
    }
    let home = app.path().home_dir().map_err(CommandError::new)?;
    let config = core::reset_config(&path, &home).map_err(|e| CommandError::new(e.message))?;
    Ok(LoadedConfig { path, config })
}

#[tauri::command]
fn change_setting(app: AppHandle, change: SettingChange) -> Result<LoadedConfig, CommandError> {
    // The describing model is switched only once it has passed its check.
    if matches!(change, SettingChange::DescribingModel(_)) {
        return Err(CommandError::new(
            "Pick the describing model under Advanced in Settings.",
        ));
    }
    save_setting(&app, &change)
}

fn save_setting(app: &AppHandle, change: &SettingChange) -> Result<LoadedConfig, CommandError> {
    let path = config_path(app)?;
    // There must be a file to change.
    read_or_create(&path)?;
    let home = app.path().home_dir().map_err(CommandError::new)?;
    let config =
        core::save_config(&path, change, &home).map_err(|e| CommandError::new(e.message))?;
    Ok(LoadedConfig { path, config })
}

/// A warning only: any folder can still be scanned.
#[tauri::command]
fn scan_root_warning(app: AppHandle, scan_root: PathBuf) -> Result<Option<String>, CommandError> {
    let loaded = read_config(&app)?;
    Ok(core::scan_root_warning(&scan_root, &loaded.config))
}

/// Called only once any protected-path warning is accepted, so this is where
/// the folder becomes a Recent Scan Root.
#[tauri::command]
async fn scan(
    app: AppHandle,
    latest: State<'_, Latest>,
    scanning: State<'_, Scanning>,
    scan_root: PathBuf,
    scan_id: u64,
    on_progress: Channel<ScanProgress>,
) -> Result<ScanResult, CommandError> {
    let cancel = {
        let mut flag = scanning.0.lock().unwrap();
        flag.id = scan_id;
        flag.cancel = Arc::new(AtomicBool::new(flag.cancelled == scan_id));
        Arc::clone(&flag.cancel)
    };
    let loaded = read_config(&app)?;
    // A list that can't be saved only loses the shortcut; the scan goes on.
    let recorded = recent_store(&app).and_then(|store| {
        core::record_recent_scan_root(&store, &scan_root).map_err(CommandError::new)
    });
    if let Err(e) = recorded {
        eprintln!("Couldn't save the Recent Scan Roots: {}", e.message);
    }
    let result = blocking(move || {
        // A closed window can't receive progress; the scan carries on.
        core::scan_with_progress(&scan_root, &loaded.config, &cancel, |progress| {
            let _ = on_progress.send(progress);
        })
    })
    .await?;
    *latest.0.lock().unwrap() = Some(Scanned {
        scan: Arc::new(result.clone()),
        duplicates: None,
        plan: None,
    });
    Ok(result)
}

/// Kept, so reading leaves out the copies to trash.
#[tauri::command]
async fn find_duplicates(
    app: AppHandle,
    latest: State<'_, Latest>,
    scanning: State<'_, Scanning>,
    on_progress: Channel<DuplicateProgress>,
) -> Result<Vec<DuplicateGroup>, CommandError> {
    // The scan's flag: a Cancel that came in since the scan ended stops this too.
    let cancel = Arc::clone(&scanning.0.lock().unwrap().cancel);
    let rule = read_config(&app)?.config.default_copy_rule;
    let scan = latest_scan(&latest)?;
    let searched = Arc::clone(&scan);
    let groups = blocking(move || {
        // A closed window can't receive progress; the search carries on.
        core::duplicate_groups(&searched, rule, &cancel, |progress| {
            let _ = on_progress.send(progress);
        })
        .ok_or("The search for duplicates was cancelled")
    })
    .await?;
    // Kept only if no newer scan came in while searching.
    if let Some(scanned) = latest
        .0
        .lock()
        .unwrap()
        .as_mut()
        .filter(|s| Arc::ptr_eq(&s.scan, &scan))
    {
        scanned.duplicates = Some(Arc::new(groups.clone()));
    }
    Ok(groups)
}

#[tauri::command]
fn cancel_scan(scanning: State<'_, Scanning>, scan_id: u64) {
    let mut flag = scanning.0.lock().unwrap();
    flag.cancelled = scan_id;
    if flag.id == scan_id {
        flag.cancel.store(true, Ordering::Relaxed);
    }
}

fn journal_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app_data_path(app, SESSIONS_FOLDER_NAME)
}

/// The Plan is kept, so a run refused as Busy can be retried.
#[tauri::command]
async fn run_plan(
    app: AppHandle,
    latest: State<'_, Latest>,
    running: State<'_, Running>,
    approved: Vec<String>,
    run_id: u64,
    on_progress: Channel<RunProgress>,
) -> Result<Summary, CommandError> {
    let cancel = {
        let mut flag = running.0.lock().unwrap();
        flag.id = run_id;
        flag.cancel = Arc::new(AtomicBool::new(flag.cancelled == run_id));
        Arc::clone(&flag.cancel)
    };
    let journal_dir = journal_dir(&app)?;
    let plan = latest
        .0
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|s| s.plan.clone());
    let plan = plan.ok_or_else(|| CommandError::new("Build a Plan first"))?;
    blocking(move || {
        // A closed window can't receive progress; the Run carries on.
        core::run_with_progress(
            &plan,
            &approved,
            &journal_dir,
            &SystemTrash,
            &cancel,
            |progress| {
                let _ = on_progress.send(progress);
            },
        )
    })
    .await
}

/// The Operation under way finishes, then everything the Run did is put back.
/// False once the Run has finished, as nothing will be put back.
#[tauri::command]
fn cancel_run(running: State<'_, Running>, run_id: u64) -> bool {
    let mut flag = running.0.lock().unwrap();
    if flag.id != run_id {
        flag.cancelled = run_id;
        return true;
    }
    !flag.cancel.swap(true, Ordering::Relaxed)
}

/// Progress is reported the way a Run reports it. There is no Cancel.
#[tauri::command]
async fn undo(
    app: AppHandle,
    session_id: String,
    operation_ids: Option<Vec<String>>,
    on_progress: Channel<RunProgress>,
) -> Result<Summary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    blocking(move || {
        core::undo_with_progress(
            &session_id,
            operation_ids.as_deref(),
            &journal_dir,
            &SystemTrash,
            |progress| {
                let _ = on_progress.send(progress);
            },
        )
    })
    .await
}

#[tauri::command]
async fn redo_operations(
    app: AppHandle,
    session_id: String,
    operation_ids: Vec<String>,
    on_progress: Channel<RunProgress>,
) -> Result<Summary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    blocking(move || {
        core::redo_with_progress(
            &session_id,
            &operation_ids,
            &journal_dir,
            &SystemTrash,
            |progress| {
                let _ = on_progress.send(progress);
            },
        )
    })
    .await
}

#[tauri::command]
async fn retry_operations(
    app: AppHandle,
    session_id: String,
    operation_ids: Vec<String>,
    on_progress: Channel<RunProgress>,
) -> Result<Summary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    blocking(move || {
        core::retry_with_progress(
            &session_id,
            &operation_ids,
            &journal_dir,
            &SystemTrash,
            |progress| {
                let _ = on_progress.send(progress);
            },
        )
    })
    .await
}

#[tauri::command]
fn session_details(app: AppHandle, session_id: String) -> Result<SessionDetails, CommandError> {
    core::session_details(&session_id, &journal_dir(&app)?).map_err(CommandError::new)
}

#[tauri::command]
fn list_sessions(app: AppHandle, offset: usize, limit: usize) -> Result<SessionPage, CommandError> {
    core::list_sessions(&journal_dir(&app)?, offset, limit).map_err(CommandError::new)
}

#[tauri::command]
fn delete_all_sessions(app: AppHandle) -> Result<usize, CommandError> {
    core::delete_all_sessions(&journal_dir(&app)?).map_err(CommandError::new)
}

fn recent_store(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app_data_path(app, RECENT_FILE_NAME)
}

#[tauri::command]
fn recent_scan_roots(app: AppHandle) -> Result<Vec<RecentScanRoot>, CommandError> {
    Ok(core::recent_scan_roots(&recent_store(&app)?))
}

#[tauri::command]
fn remove_recent_scan_root(app: AppHandle, scan_root: PathBuf) -> Result<(), CommandError> {
    core::remove_recent_scan_root(&recent_store(&app)?, &scan_root).map_err(CommandError::new)
}

fn latest_scan(latest: &Latest) -> Result<Arc<ScanResult>, CommandError> {
    let scan = latest
        .0
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| Arc::clone(&s.scan));
    scan.ok_or_else(|| CommandError::new("Scan a folder first"))
}

fn without_copies(latest: &Latest) -> Result<ScanResult, CommandError> {
    let (scan, duplicates) = {
        let guard = latest.0.lock().unwrap();
        let scanned = guard
            .as_ref()
            .ok_or_else(|| CommandError::new("Scan a folder first"))?;
        (Arc::clone(&scanned.scan), scanned.duplicates.clone())
    };
    let copies: HashSet<&Path> = duplicates
        .iter()
        .flat_map(|groups| groups.iter())
        .flat_map(|group| group.copies.iter().skip(1))
        .map(|copy| copy.path.as_path())
        .collect();
    Ok(ScanResult {
        entries: scan
            .entries
            .iter()
            .filter(|entry| !copies.contains(entry.path.as_path()))
            .cloned()
            .collect(),
        ..(*scan).clone()
    })
}

/// Optional: without PDFium or OCR's files, PDFs are read by name.
fn bundled(app: &AppHandle, path: &str) -> Option<PathBuf> {
    let path = app.path().resource_dir().ok()?.join(path);
    path.exists().then_some(path)
}

/// CI runs it on each installer it builds, to catch missing bundled files.
const SELF_CHECK_FLAG: &str = "--self-check";

const SELF_CHECK_PDF: &[u8] = b"%PDF-1.4
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 600 100]/Resources<</Font<</F1 4 0 R>>>>/Contents 5 0 R>>endobj
4 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj
5 0 obj<</Length 73>>stream
BT /F1 18 Tf 20 40 Td (Files Cleaner checks that PDFium reads this) Tj ET
endstream endobj
trailer<</Root 1 0 R>>
%%EOF
";

/// Goes through the same child process an analysis uses, so it tests the real
/// path.
fn self_check(app: &AppHandle) -> Result<(), String> {
    let pdfium = bundled(app, PDFIUM_LIBRARY)
        .ok_or_else(|| format!("{PDFIUM_LIBRARY} isn't in the app's resources"))?;
    let tessdata = bundled(app, TESSDATA_FOLDER)
        .ok_or_else(|| format!("{TESSDATA_FOLDER} isn't in the app's resources"))?;
    let languages = fs::read_dir(&tessdata)
        .map_err(|e| format!("{}: {e}", tessdata.display()))?
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "traineddata")
        })
        .count();
    if languages == 0 {
        return Err(format!("{} has no language files", tessdata.display()));
    }
    let pdf = std::env::temp_dir().join("files-cleaner-self-check.pdf");
    fs::write(&pdf, SELF_CHECK_PDF).map_err(|e| format!("{}: {e}", pdf.display()))?;
    let content = grouping::read(&pdf, "self-check.pdf", Some(&pdfium), Some(&tessdata));
    let _ = fs::remove_file(&pdf);
    if !content.body.contains("PDFium reads this") {
        return Err(format!(
            "PDFium at {} couldn't read a PDF",
            pdfium.display()
        ));
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisResult {
    grouping: Grouping,
    /// `None` when every card was stored, so no model ran.
    device: Option<String>,
}

#[tauri::command]
fn rule_overlaps(rules: Vec<core::Rule>) -> Vec<Option<usize>> {
    grouping::rule_overlaps(&rules)
}

/// By code alone and so at once: shown while the rest are read.
#[tauri::command]
async fn rule_folders(
    app: AppHandle,
    latest: State<'_, Latest>,
    models: State<'_, Arc<GroupingModels>>,
) -> Result<Grouping, CommandError> {
    let rules = read_config(&app)?.config.rules;
    let scan = without_copies(&latest)?;
    // Their files wait for the models, if they're installed.
    let by_meaning = models.installed();
    // Finding the existing folders the Rules' folders match reads the disk.
    blocking(move || Ok::<_, Infallible>(grouping::group_by_rules(&scan, &rules, by_meaning))).await
}

/// Without `read`, it groups as if stopped before the first file. Copies
/// trashed by default are left out (ADR 0007).
#[tauri::command]
async fn analyse_loose_files(
    app: AppHandle,
    latest: State<'_, Latest>,
    analysing: State<'_, Analysing>,
    models: State<'_, Arc<GroupingModels>>,
    read: bool,
    on_progress: Channel<grouping::Progress>,
) -> Result<AnalysisResult, CommandError> {
    // Set first, so a Cancel or Stop that comes in while starting isn't lost.
    let cancel = Arc::new(AtomicBool::new(false));
    *analysing.cancel.lock().unwrap() = Arc::clone(&cancel);
    let stop = Arc::new(AtomicBool::new(!read));
    *analysing.stop.lock().unwrap() = Arc::clone(&stop);
    let config = read_config(&app)?.config;
    let settings = config.grouping;
    let rules = config.rules;
    let scan = without_copies(&latest)?;
    // Without the Grouping Models, only the Rules sort files.
    if !models.installed() {
        return Ok(AnalysisResult {
            grouping: grouping::group_by_rules(&scan, &rules, false),
            device: None,
        });
    }
    let (llm, embedding) = models
        .files(config.describing_model.as_deref())
        .map_err(CommandError::new)?;
    let store = CardStore::open(app_data_path(&app, CARDS_FOLDER_NAME)?);
    let pdfium = bundled(&app, PDFIUM_LIBRARY);
    let tessdata = bundled(&app, TESSDATA_FOLDER);
    let analysis = blocking(move || {
        // Dropped when the analysis ends, freeing the models' memory.
        let mut models = LlamaModels::new(&llm, &embedding);
        let input = grouping::Input {
            scan: &scan,
            settings,
            store: &store,
            pdfium: pdfium.as_deref(),
            tessdata: tessdata.as_deref(),
            rules: &rules,
        };
        // A closed window can't receive progress; the analysis carries on.
        grouping::analyse(&input, &mut models, &cancel, &stop, |progress| {
            let _ = on_progress.send(progress);
        })
    })
    .await?;
    Ok(AnalysisResult {
        grouping: analysis.grouping,
        device: analysis.device,
    })
}

/// Hashes every file, so it can take a while; `cancel_analysis` stops it.
#[tauri::command]
async fn files_to_read(
    app: AppHandle,
    latest: State<'_, Latest>,
    analysing: State<'_, Analysing>,
    models: State<'_, Arc<GroupingModels>>,
) -> Result<usize, CommandError> {
    let cancel = Arc::new(AtomicBool::new(false));
    *analysing.cancel.lock().unwrap() = Arc::clone(&cancel);
    if !models.installed() {
        return Ok(0);
    }
    let config = read_config(&app)?.config;
    let (llm, embedding) = models
        .files(config.describing_model.as_deref())
        .map_err(CommandError::new)?;
    let scan = without_copies(&latest)?;
    let store = CardStore::open(app_data_path(&app, CARDS_FOLDER_NAME)?);
    blocking(move || {
        // Never loaded: only their names are needed, to find the stored cards.
        let models = LlamaModels::new(&llm, &embedding);
        let input = grouping::Input {
            scan: &scan,
            settings: config.grouping,
            store: &store,
            pdfium: None,
            tessdata: None,
            rules: &config.rules,
        };
        grouping::files_to_read(&input, &models, &cancel)
    })
    .await
}

#[tauri::command]
fn cancel_analysis(analysing: State<'_, Analysing>) {
    analysing
        .cancel
        .lock()
        .unwrap()
        .store(true, Ordering::Relaxed);
}

/// Unlike cancelling, the files read so far are still grouped.
#[tauri::command]
fn stop_reading(analysing: State<'_, Analysing>) {
    analysing
        .stop
        .lock()
        .unwrap()
        .store(true, Ordering::Relaxed);
}

/// Traces of earlier analyses go too.
#[tauri::command]
fn clear_grouping_cards(app: AppHandle) -> Result<(), CommandError> {
    grouping::clear_trace();
    CardStore::open(app_data_path(&app, CARDS_FOLDER_NAME)?)
        .clear()
        .map_err(CommandError::new)
}

/// A Plan built only to look at isn't kept, so it never takes the place of the
/// one about to run.
#[tauri::command]
async fn build_review_plan(
    latest: State<'_, Latest>,
    duplicates: Vec<DuplicateChoice>,
    folders: Vec<SuggestedFolderChoice>,
    keep: bool,
) -> Result<Plan, CommandError> {
    let scan = latest_scan(&latest)?;
    let planned = Arc::clone(&scan);
    // Finding names already taken, and emptied folders, reads the disk.
    let plan =
        blocking(move || core::build_review_plan(&planned, &SystemTrash, &duplicates, &folders))
            .await?;
    if !keep {
        return Ok(plan);
    }
    // Kept only if no newer scan came in while planning.
    if let Some(scanned) = latest
        .0
        .lock()
        .unwrap()
        .as_mut()
        .filter(|s| Arc::ptr_eq(&s.scan, &scan))
    {
        scanned.plan = Some(plan.clone());
    }
    Ok(plan)
}

/// Only a file of the latest scan, so the UI can't open just any path.
#[tauri::command]
fn open_file(app: AppHandle, latest: State<'_, Latest>, path: PathBuf) -> Result<(), CommandError> {
    if !latest_scan(&latest)?
        .entries
        .iter()
        .any(|entry| entry.path == path)
    {
        return Err(CommandError::new("That file isn't part of this scan."));
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(CommandError::new)
}

/// Only a path the app knows of, so the UI can't open just any path.
#[tauri::command]
fn open_session_file(
    app: AppHandle,
    session_id: String,
    operation_id: String,
) -> Result<(), CommandError> {
    let details =
        core::session_details(&session_id, &journal_dir(&app)?).map_err(CommandError::new)?;
    let op = details
        .operations
        .into_iter()
        .find(|op| op.id == operation_id)
        .ok_or_else(|| CommandError::new("That file isn't part of this Session."))?;
    let path = op
        .where_now()
        .ok_or_else(|| CommandError::new("That file can't be opened from here."))?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(CommandError::new)
}

/// Such files can't be moved there.
#[tauri::command]
fn on_another_disk(paths: Vec<PathBuf>, folder: PathBuf) -> bool {
    paths
        .iter()
        .any(|path| core::different_disks(path, &folder))
}

#[tauri::command]
fn grouping_models_status(models: State<'_, Arc<GroupingModels>>) -> models::Status {
    models.status()
}

#[tauri::command]
fn download_grouping_models(
    models: State<'_, Arc<GroupingModels>>,
) -> Result<models::Status, CommandError> {
    models.download().map_err(CommandError::new)
}

#[tauri::command]
fn pause_grouping_models(models: State<'_, Arc<GroupingModels>>) -> models::Status {
    models.pause()
}

#[tauri::command]
fn remove_grouping_models(
    models: State<'_, Arc<GroupingModels>>,
) -> Result<models::Status, CommandError> {
    models.remove().map_err(CommandError::new)
}

/// Used only once it has described a sample file.
#[tauri::command]
async fn download_describing_model(
    models: State<'_, Arc<GroupingModels>>,
    link: Option<String>,
) -> Result<models::Status, CommandError> {
    let models = Arc::clone(&models);
    // Asking Hugging Face about the file goes over the network.
    blocking(move || models.download_custom(link)).await
}

#[tauri::command]
fn pause_describing_model(models: State<'_, Arc<GroupingModels>>) -> models::Status {
    models.pause_custom()
}

#[tauri::command]
fn remove_describing_model(
    models: State<'_, Arc<GroupingModels>>,
) -> Result<models::Status, CommandError> {
    models.remove_custom().map_err(CommandError::new)
}

#[tauri::command]
fn use_describing_model(
    models: State<'_, Arc<GroupingModels>>,
    picked: bool,
) -> Result<models::Status, CommandError> {
    models.use_custom(picked).map_err(CommandError::new)
}

/// `multi_select_seen` hides the hint on selecting several items.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Onboarding {
    introduction_seen: bool,
    multi_select_seen: bool,
}

#[tauri::command]
fn onboarding(app: AppHandle) -> Result<Onboarding, CommandError> {
    let path = app_data_path(&app, ONBOARDING_FILE_NAME)?;
    // Unreadable is the same as nothing seen: the introduction shows again.
    Ok(fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default())
}

#[tauri::command]
fn save_onboarding(app: AppHandle, onboarding: Onboarding) -> Result<(), CommandError> {
    write_app_json(&app, ONBOARDING_FILE_NAME, &onboarding)
}

fn write_app_json(app: &AppHandle, name: &str, value: &impl Serialize) -> Result<(), CommandError> {
    let path = app_data_path(app, name)?;
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(CommandError::new)?;
    }
    let text = serde_json::to_string(value).map_err(CommandError::new)?;
    write_atomically(&path, text.as_bytes()).map_err(CommandError::new)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadingPace {
    /// Seconds; recent analyses weigh more (see `updatedAverage` in the UI).
    seconds_per_file: f64,
}

#[tauri::command]
fn reading_pace(app: AppHandle) -> Result<Option<f64>, CommandError> {
    let path = app_data_path(&app, READING_PACE_FILE_NAME)?;
    // Unreadable is the same as no history: the estimate starts over.
    let saved = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ReadingPace>(&bytes).ok());
    Ok(saved.map(|pace| pace.seconds_per_file))
}

#[tauri::command]
fn save_reading_pace(app: AppHandle, seconds_per_file: f64) -> Result<(), CommandError> {
    write_app_json(
        &app,
        READING_PACE_FILE_NAME,
        &ReadingPace { seconds_per_file },
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // For tuning the grouping: prompts hold file contents, so only while
    // developing (`prompts/` is ignored by git).
    #[cfg(debug_assertions)]
    grouping::trace_to(concat!(env!("CARGO_MANIFEST_DIR"), "/../prompts"));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Latest::default())
        .manage(Analysing::default())
        .manage(Scanning::default())
        .manage(Running::default())
        .setup(|app| {
            if std::env::args().any(|arg| arg == SELF_CHECK_FLAG) {
                let code = match self_check(app.handle()) {
                    Ok(()) => {
                        eprintln!("self-check passed");
                        0
                    }
                    Err(problem) => {
                        eprintln!("self-check failed: {problem}");
                        1
                    }
                };
                std::process::exit(code);
            }
            let folder = app.path().app_data_dir()?.join(MODELS_FOLDER_NAME);
            app.manage(Arc::new(GroupingModels::new(app.handle().clone(), folder)));
            menu::install(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            change_setting,
            reset_settings,
            scan_root_warning,
            scan,
            find_duplicates,
            cancel_scan,
            cancel_run,
            run_plan,
            undo,
            redo_operations,
            retry_operations,
            session_details,
            list_sessions,
            delete_all_sessions,
            recent_scan_roots,
            remove_recent_scan_root,
            grouping_models_status,
            download_grouping_models,
            pause_grouping_models,
            remove_grouping_models,
            download_describing_model,
            pause_describing_model,
            remove_describing_model,
            use_describing_model,
            onboarding,
            save_onboarding,
            rule_overlaps,
            rule_folders,
            analyse_loose_files,
            files_to_read,
            cancel_analysis,
            stop_reading,
            clear_grouping_cards,
            reading_pace,
            save_reading_pace,
            build_review_plan,
            open_file,
            open_session_file,
            on_another_disk,
            menu::set_menu_state,
            menu::show_context_menu
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
