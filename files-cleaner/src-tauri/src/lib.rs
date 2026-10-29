//! Thin Tauri commands over the core library (ADR-0003): they find the
//! config file and home folder, call the core, and serialise the result.

use std::collections::HashSet;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod models;
mod ocr_languages;

use files_cleaner_core::{
    self as core, Config, DuplicateChoice, DuplicateGroup, DuplicateProgress, Plan, RecentScanRoot,
    RunProgress, ScanProgress, ScanResult, SessionDetails, SessionPage, SettingChange,
    SuggestedFolderChoice, Summary, EXAMPLE_CONFIG,
};
use files_cleaner_grouping::{self as grouping, CardStore, Grouping, LlamaModels};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use models::GroupingModels;

const CONFIG_FILE_NAME: &str = "config.json";
/// The Journal folder, inside the app data folder.
const SESSIONS_FOLDER_NAME: &str = "sessions";
/// The Recent Scan Roots, inside the app data folder.
const RECENT_FILE_NAME: &str = "recent-scan-roots.json";
/// The Grouping Models' folder, inside the app data folder.
const MODELS_FOLDER_NAME: &str = "grouping-models";
/// The answer to the first-open offer of the Grouping Models, inside the app data folder.
const MODELS_OFFER_FILE_NAME: &str = "grouping-models-offer.json";
/// OCR's language files, inside the app data folder.
const OCR_LANGUAGES_FOLDER_NAME: &str = "ocr-languages";
/// The cards the Grouping Models wrote, inside the app data folder.
const CARDS_FOLDER_NAME: &str = "grouping-cards";
/// How long reading a file takes on this computer, inside the app data folder.
const READING_PACE_FILE_NAME: &str = "reading-pace.json";
/// The bundled PDFium library, inside the app's resources.
#[cfg(target_os = "macos")]
const PDFIUM_LIBRARY: &str = "pdfium/libpdfium.dylib";
#[cfg(target_os = "windows")]
const PDFIUM_LIBRARY: &str = "pdfium/pdfium.dll";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PDFIUM_LIBRARY: &str = "pdfium/libpdfium.so";

/// The latest scan and, once found and built, its Duplicate Groups and its
/// Plan, kept here so the UI doesn't send them back. One lock holds them
/// all, so neither outlives its scan.
#[derive(Default)]
struct Latest(Mutex<Option<Scanned>>);

struct Scanned {
    scan: Arc<ScanResult>,
    duplicates: Option<Arc<Vec<DuplicateGroup>>>,
    plan: Option<Plan>,
}

/// The cancel flag of the analysis running now, if any.
#[derive(Default)]
struct Analysing {
    cancel: Mutex<Arc<AtomicBool>>,
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
        CommandError {
            message: message.to_string(),
            config_path: None,
            line: None,
            column: None,
        }
    }
}

/// Runs `work` off the UI thread.
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

/// `name` inside the app data folder.
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

/// Reads the config file afresh, so every command sees the latest saved settings.
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

/// Makes one change from the Settings screen to the config file and
/// returns the config as saved. An invalid value is refused, with nothing
/// saved.
/// Puts every setting back to its default, even when the config can't be
/// read, and returns the config as saved.
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
    let path = config_path(&app)?;
    // Written first if missing, so there is a file to change.
    read_or_create(&path)?;
    let home = app.path().home_dir().map_err(CommandError::new)?;
    let config =
        core::save_config(&path, &change, &home).map_err(|e| CommandError::new(e.message))?;
    Ok(LoadedConfig { path, config })
}

/// Why the user should think twice before scanning `scan_root`, if they
/// should. Any folder can still be scanned.
#[tauri::command]
fn scan_root_warning(app: AppHandle, scan_root: PathBuf) -> Result<Option<String>, CommandError> {
    let loaded = read_config(&app)?;
    Ok(core::scan_root_warning(&scan_root, &loaded.config))
}

/// Scans off the UI thread, sending progress to `on_progress` as it goes.
/// The result is kept for `find_duplicates`, `analyse_loose_files` and
/// `build_review_plan`. The UI calls this only once any
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
    let recorded = recent_store(&app).and_then(|store| {
        core::record_recent_scan_root(&store, &scan_root).map_err(CommandError::new)
    });
    if let Err(e) = recorded {
        eprintln!("Couldn't save the Recent Scan Roots: {}", e.message);
    }
    let result = blocking(move || {
        // A closed window can't receive progress; the scan carries on.
        core::scan_with_progress(&scan_root, &loaded.config, |progress| {
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

/// Finds the Duplicate Groups of the latest scan, off the UI thread (it
/// reads files), sending progress to `on_progress` as it goes. They are
/// kept, so reading leaves out the copies to trash.
#[tauri::command]
async fn find_duplicates(
    latest: State<'_, Latest>,
    on_progress: Channel<DuplicateProgress>,
) -> Result<Vec<DuplicateGroup>, CommandError> {
    let scan = latest_scan(&latest)?;
    let searched = Arc::clone(&scan);
    let groups = blocking(move || {
        // A closed window can't receive progress; the search carries on.
        Ok::<_, Infallible>(core::duplicate_groups(&searched, |progress| {
            let _ = on_progress.send(progress);
        }))
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

/// Where the Journals live: a folder in the app data folder.
fn journal_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app_data_path(app, SESSIONS_FOLDER_NAME)
}

/// Runs the approved Operations of the latest Plan as a new Session, off
/// the UI thread, sending progress to `on_progress` as it goes. The Plan
/// is kept, so a run refused as Busy can be retried; running it again
/// after it ran starts a new Session whose Operations are all skipped.
#[tauri::command]
async fn run_plan(
    app: AppHandle,
    latest: State<'_, Latest>,
    approved: Vec<String>,
    on_progress: Channel<RunProgress>,
) -> Result<Summary, CommandError> {
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
        core::run_with_progress(&plan, &approved, &journal_dir, |progress| {
            let _ = on_progress.send(progress);
        })
    })
    .await
}

/// Undoes the chosen Operations of a past Session, or all of them if none
/// are chosen.
#[tauri::command]
async fn undo(
    app: AppHandle,
    session_id: String,
    operation_ids: Option<Vec<String>>,
) -> Result<Summary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    blocking(move || match operation_ids {
        Some(ids) => core::undo_operations(&session_id, &ids, &journal_dir),
        None => core::undo(&session_id, &journal_dir),
    })
    .await
}

/// Carries out again the chosen undone Operations of a past Session.
#[tauri::command]
async fn redo_operations(
    app: AppHandle,
    session_id: String,
    operation_ids: Vec<String>,
) -> Result<Summary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    blocking(move || core::redo_operations(&session_id, &operation_ids, &journal_dir)).await
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

/// The latest scan, or an error asking for one.
fn latest_scan(latest: &Latest) -> Result<Arc<ScanResult>, CommandError> {
    let scan = latest
        .0
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| Arc::clone(&s.scan));
    scan.ok_or_else(|| CommandError::new("Scan a folder first"))
}

/// The latest scan without the copies its Duplicate Groups trash by
/// default: every one but the Keeper.
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

/// PDFium as bundled with the app, if it's there; without it, PDFs are
/// read by name.
fn pdfium_library(app: &AppHandle) -> Option<PathBuf> {
    let path = app.path().resource_dir().ok()?.join(PDFIUM_LIBRARY);
    path.is_file().then_some(path)
}

/// What an analysis found, for the review screen.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisResult {
    grouping: Grouping,
    /// Where the Grouping Models ran; `None` when every card was stored.
    device: Option<String>,
}

/// Reads the loose files of the latest scan with the Grouping Models, off
/// the UI thread, sending progress to `on_progress`, and suggests folders
/// for them by the config's grouping settings. Every copy of a Duplicate
/// Group but its Keeper is left out, as it is trashed by default (ADR
/// 0005). The models are loaded for this analysis only. `cancel_analysis`
/// stops it.
#[tauri::command]
async fn analyse_loose_files(
    app: AppHandle,
    latest: State<'_, Latest>,
    analysing: State<'_, Analysing>,
    models: State<'_, Arc<GroupingModels>>,
    on_progress: Channel<grouping::Progress>,
) -> Result<AnalysisResult, CommandError> {
    // Set first, so a Cancel that comes in while starting stops this analysis.
    let cancel = Arc::new(AtomicBool::new(false));
    *analysing.cancel.lock().unwrap() = Arc::clone(&cancel);
    let settings = read_config(&app)?.config.grouping;
    let scan = without_copies(&latest)?;
    let (llm, embedding) = models
        .files()
        .ok_or_else(|| CommandError::new("Download the grouping models first"))?;
    let store = CardStore::open(app_data_path(&app, CARDS_FOLDER_NAME)?);
    let pdfium = pdfium_library(&app);
    let tessdata = ocr_languages::installed(&app_data_path(&app, OCR_LANGUAGES_FOLDER_NAME)?);
    let analysis = blocking(move || {
        // Dropped when the analysis ends, freeing the models' memory.
        let mut models = LlamaModels::new(&llm, &embedding);
        let input = grouping::Input {
            scan: &scan,
            settings,
            store: &store,
            pdfium: pdfium.as_deref(),
            tessdata: tessdata.as_deref(),
        };
        // A closed window can't receive progress; the analysis carries on.
        grouping::analyse(&input, &mut models, &cancel, |progress| {
            let _ = on_progress.send(progress);
        })
    })
    .await?;
    Ok(AnalysisResult {
        grouping: analysis.grouping,
        device: analysis.device,
    })
}

/// Stops the running analysis between files; the cards made so far are kept.
#[tauri::command]
fn cancel_analysis(analysing: State<'_, Analysing>) {
    analysing
        .cancel
        .lock()
        .unwrap()
        .store(true, Ordering::Relaxed);
}

/// Forgets the cards the Grouping Models wrote, and the traces of earlier
/// analyses, so the next analysis reads every file again.
#[tauri::command]
fn clear_grouping_cards(app: AppHandle) -> Result<(), CommandError> {
    grouping::clear_trace();
    CardStore::open(app_data_path(&app, CARDS_FOLDER_NAME)?)
        .clear()
        .map_err(CommandError::new)
}

/// Plans the user's choices on the review screen over the latest scan, as
/// one Plan: each Duplicate Group's and each Suggested Folder's. If `keep`,
/// the Plan is kept for `run_plan`, which runs it as one Session; a Plan
/// built only to look at isn't, so it never takes the place of the one
/// about to run.
#[tauri::command]
async fn build_review_plan(
    app: AppHandle,
    latest: State<'_, Latest>,
    duplicates: Vec<DuplicateChoice>,
    folders: Vec<SuggestedFolderChoice>,
    keep: bool,
) -> Result<Plan, CommandError> {
    let loaded = read_config(&app)?;
    let scan = latest_scan(&latest)?;
    let planned = Arc::clone(&scan);
    // Finding names already taken, and emptied folders, reads the disk.
    let plan =
        blocking(move || core::build_review_plan(&planned, &loaded.config, &duplicates, &folders))
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

/// The Grouping Models' status: `{ state, downloaded, total, error }`.
/// Changes are also sent as the `grouping-models` event.
#[tauri::command]
fn grouping_models_status(models: State<'_, Arc<GroupingModels>>) -> models::Status {
    models.status()
}

/// Starts or resumes downloading the Grouping Models in the background,
/// unless the disk is too full for them.
#[tauri::command]
fn download_grouping_models(
    models: State<'_, Arc<GroupingModels>>,
) -> Result<models::Status, CommandError> {
    models.download().map_err(CommandError::new)
}

/// Pauses the download, keeping what's downloaded so far.
#[tauri::command]
fn pause_grouping_models(models: State<'_, Arc<GroupingModels>>) -> models::Status {
    models.pause()
}

/// Cancels the download, or removes the installed Grouping Models.
#[tauri::command]
fn remove_grouping_models(
    models: State<'_, Arc<GroupingModels>>,
) -> Result<models::Status, CommandError> {
    models.remove().map_err(CommandError::new)
}

/// Whether to offer the Grouping Models before Home: only until the offer
/// is answered, and never once they're installed.
#[tauri::command]
fn grouping_models_offer_pending(
    app: AppHandle,
    models: State<'_, Arc<GroupingModels>>,
) -> Result<bool, CommandError> {
    Ok(!models.installed() && !app_data_path(&app, MODELS_OFFER_FILE_NAME)?.exists())
}

/// Remembers the answer to the first-open offer, `download` or `notNow`,
/// so it isn't asked again.
#[tauri::command]
fn answer_grouping_models_offer(app: AppHandle, answer: String) -> Result<(), CommandError> {
    write_app_json(
        &app,
        MODELS_OFFER_FILE_NAME,
        &serde_json::json!({ "answer": answer }),
    )
}

/// Writes `value` as JSON to the file `name` in the app data folder.
fn write_app_json(app: &AppHandle, name: &str, value: &impl Serialize) -> Result<(), CommandError> {
    let path = app_data_path(app, name)?;
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(CommandError::new)?;
    }
    let text = serde_json::to_string(value).map_err(CommandError::new)?;
    fs::write(path, text).map_err(CommandError::new)
}

/// How long reading a file takes on this computer, as stored.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadingPace {
    /// Averaged over finished analyses, each counting for half (see
    /// `updatedAverage` in the UI).
    seconds_per_file: f64,
}

/// The seconds per file a finished analysis took on this computer, or
/// `None` before the first one.
#[tauri::command]
fn reading_pace(app: AppHandle) -> Result<Option<f64>, CommandError> {
    let path = app_data_path(&app, READING_PACE_FILE_NAME)?;
    // Unreadable is the same as no history: the estimate starts over.
    let saved = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ReadingPace>(&bytes).ok());
    Ok(saved.map(|pace| pace.seconds_per_file))
}

/// Remembers `seconds_per_file` as this computer's reading pace.
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
    // While developing, each prompt and reply and a similarity report go to
    // `files-cleaner/prompts/` (ignored by git), for tuning the grouping.
    #[cfg(debug_assertions)]
    grouping::trace_to(concat!(env!("CARGO_MANIFEST_DIR"), "/../prompts"));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Latest::default())
        .manage(Analysing::default())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            ocr_languages::install(data.join(OCR_LANGUAGES_FOLDER_NAME));
            let folder = data.join(MODELS_FOLDER_NAME);
            app.manage(Arc::new(GroupingModels::new(app.handle().clone(), folder)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            change_setting,
            reset_settings,
            scan_root_warning,
            scan,
            find_duplicates,
            run_plan,
            undo,
            redo_operations,
            session_details,
            list_sessions,
            recent_scan_roots,
            remove_recent_scan_root,
            grouping_models_status,
            download_grouping_models,
            pause_grouping_models,
            remove_grouping_models,
            grouping_models_offer_pending,
            answer_grouping_models_offer,
            analyse_loose_files,
            cancel_analysis,
            clear_grouping_cards,
            reading_pace,
            save_reading_pace,
            build_review_plan
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
