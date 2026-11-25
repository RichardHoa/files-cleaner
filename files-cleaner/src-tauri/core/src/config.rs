use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::atomic_file;
use crate::rules::{rule_problem, Rule};

pub const EXAMPLE_CONFIG: &str = r#"{
  "neverTouch": []
}
"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub home: PathBuf,
    pub never_touch: Vec<PathBuf>,
    pub default_copy_rule: DefaultCopyRule,
    pub grouping: GroupingSettings,
    /// `None` until the user is asked, which is only once.
    pub notifications: Option<bool>,
    /// A Hugging Face link, or `None` for the app's own model.
    pub describing_model: Option<String>,
    /// In order of priority: the first enabled one that matches a file wins.
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupingSettings {
    /// 0–1; higher is stricter.
    pub folder_threshold: f32,
    /// 0–1; higher is stricter.
    pub existing_folder_threshold: f32,
    /// 2–10.
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", tag = "key", content = "value")]
pub enum SettingChange {
    NeverTouch(Vec<PathBuf>),
    DefaultCopyRule(DefaultCopyRule),
    FolderThreshold(Option<f32>),
    ExistingFolderThreshold(Option<f32>),
    SmallestFolder(Option<usize>),
    Notifications(bool),
    /// Set by the app only once the model has passed its check.
    DescribingModel(Option<String>),
    Rules(Vec<Rule>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DefaultCopyRule {
    #[default]
    Newest,
    Oldest,
    ShortestPath,
}

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
    /// Retired: Trash always goes to the System Trash now. Still accepted so
    /// older configs load.
    #[serde(default, rename = "trashDestination")]
    _trash_destination: serde::de::IgnoredAny,
    /// Retired: running or skipping the duplicate step is the choice now. Still
    /// accepted so older configs load.
    #[serde(default, rename = "duplicates")]
    _duplicates: serde::de::IgnoredAny,
    #[serde(default)]
    never_touch: Vec<String>,
    #[serde(default)]
    default_copy_rule: DefaultCopyRule,
    /// The Rules of ADR 0008, retired. Still accepted so older configs load.
    #[serde(default, rename = "rules")]
    _rules: serde::de::IgnoredAny,
    #[serde(default)]
    grouping: RawGrouping,
    notifications: Option<bool>,
    describing_model: Option<String>,
    /// Apart from the retired `rules` key, whose values mean something else
    /// (ADR 0013).
    #[serde(default)]
    sorting_rules: Vec<Rule>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawGrouping {
    folder_threshold: Option<f32>,
    existing_folder_threshold: Option<f32>,
    smallest_folder: Option<usize>,
}

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
    if let Some(problem) = raw.sorting_rules.iter().find_map(rule_problem) {
        let section = text.find("\"sortingRules\"").unwrap_or(0);
        return Err(error_at(text, section, "sortingRules", problem));
    }
    Ok(Config {
        home: home.to_path_buf(),
        never_touch,
        default_copy_rule: raw.default_copy_rule,
        grouping,
        notifications: raw.notifications,
        describing_model: raw.describing_model,
        rules: raw.sorting_rules,
    })
}

/// An out-of-range value is an error, never clamped, so the user's setting
/// isn't silently changed.
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

/// Untouched keys stay as written, and a change that would make the config
/// invalid is refused before anything is written.
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
            // Not expanded, so a path the user wrote as `~/Keep` stays that
            // way.
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
        SettingChange::Rules(rules) if rules.is_empty() => {
            object.remove("sortingRules");
        }
        SettingChange::Rules(rules) => {
            let rules = serde_json::to_value(rules).map_err(|e| plain(&e))?;
            object.insert("sortingRules".into(), rules);
        }
    }
    let mut saved = serde_json::to_string_pretty(&json).map_err(|e| plain(&e))?;
    saved.push('\n');
    // Positions in text that was never written would only mislead.
    let config = load_config(&saved, home).map_err(|e| plain(&e.message))?;
    write_atomically(path, &saved)?;
    Ok(config)
}

/// Never reads the old file, so it works even when that is broken.
pub fn reset_config(path: &Path, home: &Path) -> Result<Config, ConfigError> {
    write_atomically(path, EXAMPLE_CONFIG)?;
    load_config(EXAMPLE_CONFIG, home)
}

fn write_atomically(path: &Path, text: &str) -> Result<(), ConfigError> {
    atomic_file::write_atomically(path, text.as_bytes()).map_err(|e| ConfigError {
        message: e.to_string(),
        line: None,
        column: None,
    })
}

/// As a string first: the nearest double to 0.73f32 would be saved as
/// 0.7300000190734863.
fn threshold_json(value: f32) -> serde_json::Value {
    value
        .to_string()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map_or(serde_json::Value::Null, serde_json::Value::Number)
}

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

/// `None` if it is relative. `~` is the home folder on every OS, so a config
/// can move between machines.
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

fn find(text: &str, from: usize, value: &str) -> Option<usize> {
    let quoted = serde_json::to_string(value).ok()?;
    text.get(from..)?.find(&quoted).map(|offset| from + offset)
}
