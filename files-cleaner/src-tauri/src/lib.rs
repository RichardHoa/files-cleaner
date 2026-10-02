//! Thin Tauri commands over the core library (ADR-0003): they find the
//! config file and home folder, call the core, and serialise the result.

use std::fs;
use std::path::{Path, PathBuf};

use files_cleaner_core::{self as core, Config, ScanResult, EXAMPLE_CONFIG};
use serde::Serialize;
use tauri::{AppHandle, Manager};

const CONFIG_FILE_NAME: &str = "config.json";

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

fn config_path(app: &AppHandle) -> Result<PathBuf, CommandError> {
    let folder = app.path().app_data_dir().map_err(CommandError::new)?;
    Ok(folder.join(CONFIG_FILE_NAME))
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

#[tauri::command]
async fn scan(app: AppHandle, scan_root: PathBuf) -> Result<ScanResult, CommandError> {
    let loaded = read_config(&app)?;
    tauri::async_runtime::spawn_blocking(move || core::scan(&scan_root, &loaded.config))
        .await
        .map_err(CommandError::new)?
        .map_err(CommandError::new)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![load_config, open_config, scan])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
