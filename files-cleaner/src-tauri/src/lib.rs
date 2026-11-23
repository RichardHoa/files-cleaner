//! Thin Tauri commands over the core library (ADR-0003): they find the
//! config file and home folder, call the core, and serialise the result.

use std::collections::HashSet;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod downloads;
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
/// The Journal folder, inside the app data folder.
const SESSIONS_FOLDER_NAME: &str = "sessions";
/// The Recent Scan Roots, inside the app data folder.
const RECENT_FILE_NAME: &str = "recent-scan-roots.json";
/// The Grouping Models' folder, inside the app data folder.
const MODELS_FOLDER_NAME: &str = "grouping-models";
/// What the user has seen of the introduction, inside the app data folder.
const ONBOARDING_FILE_NAME: &str = "onboarding.json";
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
/// OCR's language files, bundled by the build script, inside the app's resources.
const TESSDATA_FOLDER: &str = "tessdata";

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

/// The latest scan, by the ID the UI gave it, with the cancel flag it and
/// the search for duplicates after it share.
#[derive(Default)]
struct Scanning(Mutex<ScanFlag>);

#[derive(Default)]
struct ScanFlag {
    id: u64,
    cancel: Arc<AtomicBool>,
    /// The scan last cancelled: a Cancel can reach the app before its scan
    /// has started, and that scan then starts cancelled.
    cancelled: u64,
}

/// The cancel and stop flags of the analysis running now, if any.
#[derive(Default)]
struct Analysing {
    cancel: Mutex<Arc<AtomicBool>>,
    stop: Mutex<Arc<AtomicBool>>,
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
        write_atomically(path, EXAMPLE_CONFIG.as_bytes()).map_err(CommandError::new)?;
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
    // The describing model is switched only once it has passed its check.
    if matches!(change, SettingChange::DescribingModel(_)) {
        return Err(CommandError::new(
            "Pick the describing model under Advanced in Settings.",
        ));
    }
    save_setting(&app, &change)
}

/// Makes `change` to the config file and returns the config as saved.
fn save_setting(app: &AppHandle, change: &SettingChange) -> Result<LoadedConfig, CommandError> {
    let path = config_path(app)?;
    // Written first if missing, so there is a file to change.
    read_or_create(&path)?;
    let home = app.path().home_dir().map_err(CommandError::new)?;
    let config =
        core::save_config(&path, change, &home).map_err(|e| CommandError::new(e.message))?;
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

/// Finds the Duplicate Groups of the latest scan, off the UI thread (it
/// reads files), sending progress to `on_progress` as it goes. They are
/// kept, so reading leaves out the copies to trash. `cancel_scan` stops it,
/// like the scan before it.
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

/// Stops the scan `scan_id`, or the search for duplicates after it, between
/// files, even if it hasn't started yet; nothing it found is kept.
#[tauri::command]
fn cancel_scan(scanning: State<'_, Scanning>, scan_id: u64) {
    let mut flag = scanning.0.lock().unwrap();
    flag.cancelled = scan_id;
    if flag.id == scan_id {
        flag.cancel.store(true, Ordering::Relaxed);
    }
}

/// Where the Journals live: a folder in the app data folder.
fn journal_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app_data_path(app, SESSIONS_FOLDER_NAME)
}

/// Runs the approved Operations of the latest Plan as a new Session, off
/// the UI thread, sending progress to `on_progress` as it goes. The Plan
/// is kept, so a run refused as Busy can be retried; running it again
/// after it ran does nothing, so it keeps no Session.
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
        core::run_with_progress(&plan, &approved, &journal_dir, &SystemTrash, |progress| {
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
        Some(ids) => core::undo_operations(&session_id, &ids, &journal_dir, &SystemTrash),
        None => core::undo(&session_id, &journal_dir, &SystemTrash),
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
    blocking(move || core::redo_operations(&session_id, &operation_ids, &journal_dir, &SystemTrash))
        .await
}

/// Runs again the chosen skipped or failed Operations of a past Session, in
/// that Session.
#[tauri::command]
async fn retry_operations(
    app: AppHandle,
    session_id: String,
    operation_ids: Vec<String>,
) -> Result<Summary, CommandError> {
    let journal_dir = journal_dir(&app)?;
    blocking(move || {
        core::retry_operations(&session_id, &operation_ids, &journal_dir, &SystemTrash)
    })
    .await
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

/// Deletes every past Session's Journal, clearing History and changing no
/// file of the user's. Refused as Busy during a Run, Undo or Redo.
#[tauri::command]
fn delete_all_sessions(app: AppHandle) -> Result<usize, CommandError> {
    core::delete_all_sessions(&journal_dir(&app)?).map_err(CommandError::new)
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
/// default: every one but the first.
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

/// The file or folder at `path` in the app's resources, if it's there:
/// PDFium (without it, PDFs are read by name) or OCR's language files
/// (without them, scanned PDFs are).
fn bundled(app: &AppHandle, path: &str) -> Option<PathBuf> {
    let path = app.path().resource_dir().ok()?.join(path);
    path.exists().then_some(path)
}

/// Started with this flag, the app checks what it was installed with, says
/// how that went on stderr and exits: 0 if all is well. CI runs it on each
/// installer it builds.
const SELF_CHECK_FLAG: &str = "--self-check";

/// A one-page PDF with a line of text, for PDFium to read in the self-check.
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

/// Whether the installed app has its bundled resources and can use them:
/// PDFium reads a PDF, through the same child process an analysis uses, and
/// OCR's language files are there.
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

/// What an analysis found, for the review screen.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisResult {
    grouping: Grouping,
    /// Where the Grouping Models ran; `None` when every card was stored.
    device: Option<String>,
}

/// For each of `rules`, the first enabled Rule before it that some file
/// could match too, if any: that file goes where the earlier one says.
#[tauri::command]
fn rule_overlaps(rules: Vec<core::Rule>) -> Vec<Option<usize>> {
    grouping::rule_overlaps(&rules)
}

/// The Suggested Folders the config's Rules make of the loose files of the
/// latest scan, by code alone and so at once: shown while the rest are read.
/// Every copy of a Duplicate Group but its first is left out, as for
/// `analyse_loose_files`.
#[tauri::command]
async fn rule_folders(
    app: AppHandle,
    latest: State<'_, Latest>,
    models: State<'_, Arc<GroupingModels>>,
) -> Result<Grouping, CommandError> {
    let rules = read_config(&app)?.config.rules;
    let scan = without_copies(&latest)?;
    // A Rule by meaning's files wait for the models, if they're in.
    let by_meaning = models.installed();
    // Finding the existing folders the Rules' folders match reads the disk.
    blocking(move || Ok::<_, Infallible>(grouping::group_by_rules(&scan, &rules, by_meaning))).await
}

/// Reads the loose files of the latest scan with the Grouping Models, off
/// the UI thread, sending progress to `on_progress`, and suggests folders
/// for them by the config's Rules, then its grouping settings. Without the
/// models, only the Rules suggest folders and nothing is read. Every copy of a Duplicate
/// Group but its first is left out, as it is trashed by default (ADR
/// 0007). The models are loaded for this analysis only. `cancel_analysis`
/// stops it; `stop_reading` ends its reading early and groups what was read.
/// Unless `read`, nothing is read: it groups as if stopped before the first file.
#[tauri::command]
async fn analyse_loose_files(
    app: AppHandle,
    latest: State<'_, Latest>,
    analysing: State<'_, Analysing>,
    models: State<'_, Arc<GroupingModels>>,
    read: bool,
    on_progress: Channel<grouping::Progress>,
) -> Result<AnalysisResult, CommandError> {
    // Set first, so a Cancel or Stop that comes in while starting stops this analysis.
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

/// How many loose files of the latest scan the Grouping Models would read,
/// leaving out those read before; 0 without the models. It hashes every
/// file, off the UI thread; `cancel_analysis` stops it.
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

/// Stops the running analysis between files; the cards made so far are kept.
#[tauri::command]
fn cancel_analysis(analysing: State<'_, Analysing>) {
    analysing
        .cancel
        .lock()
        .unwrap()
        .store(true, Ordering::Relaxed);
}

/// Ends the running analysis's reading between files: the files read so far
/// are grouped and the rest are left where they are.
#[tauri::command]
fn stop_reading(analysing: State<'_, Analysing>) {
    analysing
        .stop
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

/// Opens `path` in its default app. Only a file of the latest scan can be
/// opened, so the UI can't open just any path.
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

/// Opens, in its default app, where the item of the Operation `operation_id`
/// of a past Session is now. Like `open_file`, it only opens a path the app
/// knows of: none while the item is in the System Trash or is gone for good.
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

/// Whether any of the files at `paths` is on a different disk from `folder`.
#[tauri::command]
fn on_another_disk(paths: Vec<PathBuf>, folder: PathBuf) -> bool {
    paths
        .iter()
        .any(|path| core::different_disks(path, &folder))
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

/// Starts downloading the describing model at the Hugging Face `link`, in
/// place of any picked before, or resumes the one stopped when `link` is
/// null. Once downloaded it describes a sample file, and is used
/// only if it can.
#[tauri::command]
async fn download_describing_model(
    models: State<'_, Arc<GroupingModels>>,
    link: Option<String>,
) -> Result<models::Status, CommandError> {
    let models = Arc::clone(&models);
    // Asking Hugging Face about the file goes over the network.
    blocking(move || models.download_custom(link)).await
}

/// Pauses the picked describing model's download.
#[tauri::command]
fn pause_describing_model(models: State<'_, Arc<GroupingModels>>) -> models::Status {
    models.pause_custom()
}

/// Cancels the picked describing model's download, or deletes it.
#[tauri::command]
fn remove_describing_model(
    models: State<'_, Arc<GroupingModels>>,
) -> Result<models::Status, CommandError> {
    models.remove_custom().map_err(CommandError::new)
}

/// Describes files with the picked model if `picked`, or else the default.
#[tauri::command]
fn use_describing_model(
    models: State<'_, Arc<GroupingModels>>,
    picked: bool,
) -> Result<models::Status, CommandError> {
    models.use_custom(picked).map_err(CommandError::new)
}

/// What the user has seen of the introduction, as stored.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Onboarding {
    introduction_seen: bool,
}

/// What the user has seen of the introduction.
#[tauri::command]
fn onboarding(app: AppHandle) -> Result<Onboarding, CommandError> {
    let path = app_data_path(&app, ONBOARDING_FILE_NAME)?;
    // Unreadable is the same as nothing seen: the introduction shows again.
    Ok(fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default())
}

/// Remembers what the user has seen of the introduction.
#[tauri::command]
fn save_onboarding(app: AppHandle, onboarding: Onboarding) -> Result<(), CommandError> {
    write_app_json(&app, ONBOARDING_FILE_NAME, &onboarding)
}

/// Writes `value` as JSON to the file `name` in the app data folder.
fn write_app_json(app: &AppHandle, name: &str, value: &impl Serialize) -> Result<(), CommandError> {
    let path = app_data_path(app, name)?;
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(CommandError::new)?;
    }
    let text = serde_json::to_string(value).map_err(CommandError::new)?;
    write_atomically(&path, text.as_bytes()).map_err(CommandError::new)
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
        .manage(Scanning::default())
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
            on_another_disk
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
