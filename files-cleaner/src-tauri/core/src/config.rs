use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The config written on first launch, as a working starting point.
pub const EXAMPLE_CONFIG: &str = r#"{
  "neverTouch": ["~/Downloads/Keep"]
}
"#;

/// A validated config file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// The user's home folder, which `~/` paths were expanded against.
    pub home: PathBuf,
    pub never_touch: Vec<PathBuf>,
    pub default_copy_rule: DefaultCopyRule,
    pub grouping: GroupingSettings,
    /// Whether the user wants a notification when the folders are ready:
    /// `None` until they're asked, which is only once.
    pub notifications: Option<bool>,
    /// The Hugging Face link of the model the user picked to describe
    /// files (ADR 0009), or `None` for the app's own.
    pub describing_model: Option<String>,
}

/// How the files are grouped into Suggested Folders: the `grouping` section,
/// each key defaulting to the value here.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupingSettings {
    /// How closely files must match to share a folder, 0–1.
    pub folder_threshold: f32,
    /// How closely a group must match an existing subfolder to go into it, 0–1.
    pub existing_folder_threshold: f32,
    /// The fewest files a Suggested Folder holds, 2–10.
    pub smallest_folder: usize,
}

impl Default for GroupingSettings {
    fn default() -> Self {
        GroupingSettings {
            folder_threshold: 0.86,
            existing_folder_threshold: 0.86,
            smallest_folder: 2,
        }
    }
}

/// One setting changed from the Settings screen. A grouping value of `None`
/// resets it to the default, taking its key out of the file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", tag = "key", content = "value")]
pub enum SettingChange {
    NeverTouch(Vec<PathBuf>),
    DefaultCopyRule(DefaultCopyRule),
    FolderThreshold(Option<f32>),
    ExistingFolderThreshold(Option<f32>),
    SmallestFolder(Option<usize>),
    /// The user's answer when asked about notifications.
    Notifications(bool),
    /// The Hugging Face link of the model that describes files, or `None`
    /// for the app's own. Set by the app once the model has passed its check.
    DescribingModel(Option<String>),
}

/// Which copy of a Duplicate Group starts out kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DefaultCopyRule {
    #[default]
    Newest,
    Oldest,
    ShortestPath,
}

/// Why a config couldn't be loaded, with where in the file when known.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConfigError {
    pub message: String,
    /// 1-based.
    pub line: Option<usize>,
    /// 1-based.
    pub column: Option<usize>,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(column)) => {
                write!(f, "line {line}, column {column}: {}", self.message)
            }
            _ => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawConfig {
    /// The old Trash Destination setting: Trash always goes to the System
    /// Trash now, so any value is ignored.
    #[serde(default, rename = "trashDestination")]
    _trash_destination: serde::de::IgnoredAny,
    /// The old Duplicate Handling setting: running or skipping the
    /// duplicate step is the choice now, so any value is ignored.
    #[serde(default, rename = "duplicates")]
    _duplicates: serde::de::IgnoredAny,
    #[serde(default)]
    never_touch: Vec<String>,
    #[serde(default)]
    default_copy_rule: DefaultCopyRule,
    /// The old Rules (ADR 0008): any value is ignored.
    #[serde(default, rename = "rules")]
    _rules: serde::de::IgnoredAny,
    #[serde(default)]
    grouping: RawGrouping,
    notifications: Option<bool>,
    describing_model: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawGrouping {
    folder_threshold: Option<f32>,
    existing_folder_threshold: Option<f32>,
    smallest_folder: Option<usize>,
}

/// Parses and validates the config JSON. `home` expands `~/` paths.
pub fn load_config(text: &str, home: &Path) -> Result<Config, ConfigError> {
    let raw: RawConfig = serde_json::from_str(text).map_err(json_error)?;
    let mut never_touch = Vec::new();
    for written in &raw.never_touch {
        match expand_home(written, home) {
            Some(path) => never_touch.push(path),
            None => {
                return Err(error_at(
                    text,
                    0,
                    written,
                    format!("Never-Touch Path \"{written}\" must be a full path or start with ~/"),
                ))
            }
        }
    }

    let grouping = validate_grouping(&raw.grouping, text)?;
    if let Some(link) = &raw.describing_model {
        crate::describing_model_link(link).map_err(|message| error_at(text, 0, link, message))?;
    }
    Ok(Config {
        home: home.to_path_buf(),
        never_touch,
        default_copy_rule: raw.default_copy_rule,
        grouping,
        notifications: raw.notifications,
        describing_model: raw.describing_model,
    })
}

/// The grouping settings, with the defaults for keys left out. An
/// out-of-range value is an error at its key, never clamped.
fn validate_grouping(raw: &RawGrouping, text: &str) -> Result<GroupingSettings, ConfigError> {
    let section = text.find("\"grouping\"").unwrap_or(0);
    let fail = |key: &str, range: &str| {
        error_at(
            text,
            section,
            key,
            format!("grouping {key} must be between {range}"),
        )
    };
    let defaults = GroupingSettings::default();
    let threshold = |value: Option<f32>, key: &str, default: f32| match value {
        Some(value) if !(0.0..=1.0).contains(&value) => Err(fail(key, "0 and 1")),
        value => Ok(value.unwrap_or(default)),
    };
    let smallest_folder = match raw.smallest_folder {
        Some(value) if !(2..=10).contains(&value) => {
            return Err(fail("smallestFolder", "2 and 10"))
        }
        value => value.unwrap_or(defaults.smallest_folder),
    };
    Ok(GroupingSettings {
        folder_threshold: threshold(
            raw.folder_threshold,
            "folderThreshold",
            defaults.folder_threshold,
        )?,
        existing_folder_threshold: threshold(
            raw.existing_folder_threshold,
            "existingFolderThreshold",
            defaults.existing_folder_threshold,
        )?,
        smallest_folder,
    })
}

/// Makes `change` to the config file at `path` and returns the config it
/// now holds. Every key the change doesn't touch is kept as written. A
/// change that would make the config invalid is refused and nothing is
/// written; otherwise the file is replaced atomically, through a temporary
/// file and a rename.
pub fn save_config(
    path: &Path,
    change: &SettingChange,
    home: &Path,
) -> Result<Config, ConfigError> {
    let plain = |e: &dyn fmt::Display| ConfigError {
        message: e.to_string(),
        line: None,
        column: None,
    };
    let text = fs::read_to_string(path).map_err(|e| plain(&e))?;
    let current = load_config(&text, home)?;
    let mut json: serde_json::Value = serde_json::from_str(&text).map_err(json_error)?;
    let object = json
        .as_object_mut()
        .ok_or_else(|| plain(&"the config must be a JSON object"))?;
    match change {
        SettingChange::NeverTouch(paths) => {
            // Paths kept from before keep the way they were written, such as `~/Keep`.
            let written: Vec<_> = object
                .get("neverTouch")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .zip(&current.never_touch)
                .collect();
            let list = paths
                .iter()
                .map(|path| {
                    serde_json::Value::String(
                        written
                            .iter()
                            .find(|(_, expanded)| *expanded == path)
                            .map_or_else(
                                || path.to_string_lossy().into_owned(),
                                |(w, _)| w.to_string(),
                            ),
                    )
                })
                .collect();
            object.insert("neverTouch".into(), serde_json::Value::Array(list));
        }
        SettingChange::DefaultCopyRule(rule) => {
            let rule = serde_json::to_value(rule).map_err(|e| plain(&e))?;
            object.insert("defaultCopyRule".into(), rule);
        }
        SettingChange::FolderThreshold(value) => {
            set_grouping(object, "folderThreshold", value.map(threshold_json))
        }
        SettingChange::ExistingFolderThreshold(value) => {
            set_grouping(object, "existingFolderThreshold", value.map(threshold_json))
        }
        SettingChange::SmallestFolder(value) => {
            set_grouping(object, "smallestFolder", value.map(serde_json::Value::from))
        }
        SettingChange::Notifications(wanted) => {
            object.insert("notifications".into(), (*wanted).into());
        }
        SettingChange::DescribingModel(Some(link)) => {
            object.insert("describingModel".into(), link.as_str().into());
        }
        SettingChange::DescribingModel(None) => {
            object.remove("describingModel");
        }
    }
    let mut saved = serde_json::to_string_pretty(&json).map_err(|e| plain(&e))?;
    saved.push('\n');
    // Positions in text that was never written would only mislead.
    let config = load_config(&saved, home).map_err(|e| plain(&e.message))?;
    write_atomically(path, &saved)?;
    Ok(config)
}

/// Puts the config file at `path` back to the example config, whatever is
/// in it now, even if it can't be read, and returns that config.
pub fn reset_config(path: &Path, home: &Path) -> Result<Config, ConfigError> {
    write_atomically(path, EXAMPLE_CONFIG)?;
    load_config(EXAMPLE_CONFIG, home)
}

/// Replaces the file at `path` with `text` through a temporary file and a
/// rename, so a crash mid-write leaves the old file rather than half a new one.
fn write_atomically(path: &Path, text: &str) -> Result<(), ConfigError> {
    let plain = |e: std::io::Error| ConfigError {
        message: e.to_string(),
        line: None,
        column: None,
    };
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, text).map_err(plain)?;
    fs::rename(&temporary, path).map_err(plain)
}

/// A threshold as JSON, written as it reads (`0.73`), not as the nearest
/// double to the `f32` (`0.7300000190734863`). Not-a-number becomes `null`,
/// which the config then rejects.
fn threshold_json(value: f32) -> serde_json::Value {
    value
        .to_string()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map_or(serde_json::Value::Null, serde_json::Value::Number)
}

/// Sets `key` of the `grouping` section to `value`, or takes it out when
/// `value` is `None`; a section left empty goes too.
fn set_grouping(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<serde_json::Value>,
) {
    let section = object
        .entry("grouping")
        .or_insert_with(|| serde_json::Value::Object(Default::default()));
    if let Some(section) = section.as_object_mut() {
        match value {
            Some(value) => {
                section.insert(key.into(), value);
            }
            None => {
                section.remove(key);
            }
        }
        if section.is_empty() {
            object.remove("grouping");
        }
    }
}

/// The absolute path `written` names, or `None` if it is relative. `~`
/// is the user's home folder on every OS; on Windows `~\` works too, as do
/// `C:\…` paths.
fn expand_home(written: &str, home: &Path) -> Option<PathBuf> {
    let after_tilde = written.strip_prefix('~');
    if after_tilde == Some("") {
        Some(home.to_path_buf())
    } else if let Some(rest) =
        after_tilde.and_then(|rest| rest.strip_prefix(std::path::is_separator))
    {
        Some(home.join(rest))
    } else if Path::new(written).is_absolute() {
        Some(PathBuf::from(written))
    } else {
        None
    }
}

fn json_error(e: serde_json::Error) -> ConfigError {
    let (line, column) = (e.line(), e.column());
    // serde_json appends the position to its message; it has its own fields.
    let message = e.to_string();
    let message = message
        .strip_suffix(&format!(" at line {line} column {column}"))
        .unwrap_or(&message)
        .to_string();
    if line == 0 {
        ConfigError {
            message,
            line: None,
            column: None,
        }
    } else {
        ConfigError {
            message,
            line: Some(line),
            column: Some(column),
        }
    }
}

/// An error placed where the string `value` next appears in the file,
/// searching from byte `from`.
fn error_at(text: &str, from: usize, value: &str, message: String) -> ConfigError {
    let position = find(text, from, value).map(|offset| {
        let before = &text[..offset];
        let line = before.matches('\n').count() + 1;
        let column = before.len() - before.rfind('\n').map_or(0, |i| i + 1) + 1;
        (line, column)
    });
    ConfigError {
        message,
        line: position.map(|p| p.0),
        column: position.map(|p| p.1),
    }
}

/// The byte offset of the JSON string `value` in `text`, from byte `from`.
fn find(text: &str, from: usize, value: &str) -> Option<usize> {
    let quoted = serde_json::to_string(value).ok()?;
    text.get(from..)?.find(&quoted).map(|offset| from + offset)
}
