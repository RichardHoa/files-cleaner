use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::protection::Protection;

/// The config written on first launch, as a working starting point.
pub const EXAMPLE_CONFIG: &str = r#"{
  "trashDestination": "holdingFolder",
  "duplicates": "trash",
  "neverTouch": ["~/Downloads/Keep"],
  "rules": [
    { "name": "Disk images", "match": { "extensions": ["dmg"] }, "then": { "trash": true } },
    { "name": "Slides", "match": { "extensions": ["pptx", "key"] }, "then": { "moveTo": "~/Documents/Study" } },
    { "name": "Screenshots", "match": { "nameGlob": "Screenshot*" },
      "then": { "moveTo": "Screenshots", "rename": "{modified} {name}" } }
  ]
}
"#;

/// A validated config file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// The user's home folder, which `~/` paths were expanded against.
    pub home: PathBuf,
    pub trash_destination: TrashDestination,
    pub duplicates: DuplicateHandling,
    pub never_touch: Vec<PathBuf>,
    /// In file order: the first matching Rule wins.
    pub rules: Vec<Rule>,
}

/// Where Trash sends files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrashDestination {
    #[default]
    HoldingFolder,
    SystemTrash,
}

/// What happens to the non-Keeper copies in a Duplicate Group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateHandling {
    #[default]
    Trash,
    FollowRules,
    Leave,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub name: String,
    /// Lowercase, without the dot. Empty means "any extension".
    pub extensions: Vec<String>,
    pub name_glob: Option<String>,
    pub operation: RuleOperation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleOperation {
    Trash,
    MoveTo { folder: RuleTarget, rename: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleTarget {
    /// Written as an absolute or `~/` path.
    Absolute(PathBuf),
    /// Written as a relative path; resolved against the Scan Root when
    /// planning.
    RelativeToScanRoot(PathBuf),
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
            (Some(line), Some(column)) => write!(f, "line {line}, column {column}: {}", self.message),
            _ => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    trash_destination: TrashDestination,
    #[serde(default)]
    duplicates: DuplicateHandling,
    #[serde(default)]
    never_touch: Vec<String>,
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    name: String,
    #[serde(rename = "match")]
    matcher: RawMatch,
    then: RawThen,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawMatch {
    #[serde(default)]
    extensions: Vec<String>,
    name_glob: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawThen {
    trash: Option<bool>,
    move_to: Option<String>,
    rename: Option<String>,
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
                    format!("Never-Touch Path \"{written}\" must start with / or ~/"),
                ))
            }
        }
    }

    let mut config = Config {
        home: home.to_path_buf(),
        trash_destination: raw.trash_destination,
        duplicates: raw.duplicates,
        never_touch,
        rules: Vec::new(),
    };
    let protection = Protection::new(&config);
    // Rules are in file order, so searching onwards from the previous
    // Rule's name finds each Rule's own text, not an earlier copy of it.
    let mut cursor = text.find("\"rules\"").unwrap_or(0);
    for rule in raw.rules {
        let start = find(text, cursor, &rule.name).unwrap_or(cursor);
        config.rules.push(validate_rule(rule, text, start, home, &protection)?);
        cursor = start + 1;
    }
    Ok(config)
}

/// Checks a Rule's shape and target. Errors point at the Rule's name, or at
/// its target when that is the problem.
fn validate_rule(
    raw: RawRule,
    text: &str,
    start: usize,
    home: &Path,
    protection: &Protection,
) -> Result<Rule, ConfigError> {
    let name = raw.name;
    let fail = |at: &str, problem: String| Err(error_at(text, start, at, format!("Rule \"{name}\": {problem}")));
    if raw.matcher.extensions.is_empty() && raw.matcher.name_glob.is_none() {
        return fail(&name, "match needs extensions or nameGlob".into());
    }
    let operation = match raw.then {
        RawThen { trash: Some(true), move_to: None, rename: None } => RuleOperation::Trash,
        RawThen { trash: None, move_to: Some(written), rename } => {
            let folder = match expand_home(&written, home) {
                Some(path) => match protection.refusal(&path) {
                    Some(reason) => return fail(&written, format!("moveTo {reason}")),
                    None => RuleTarget::Absolute(path),
                },
                None => RuleTarget::RelativeToScanRoot(PathBuf::from(written)),
            };
            RuleOperation::MoveTo { folder, rename }
        }
        _ => {
            return fail(
                &name,
                "then must be either { \"trash\": true } or { \"moveTo\": \"folder\" } with an optional rename".into(),
            )
        }
    };
    let extensions = raw
        .matcher
        .extensions
        .iter()
        .map(|e| e.trim_start_matches('.').to_lowercase())
        .collect();
    Ok(Rule { name, extensions, name_glob: raw.matcher.name_glob, operation })
}

/// The absolute path `written` names, or `None` if it is relative.
fn expand_home(written: &str, home: &Path) -> Option<PathBuf> {
    if written == "~" {
        Some(home.to_path_buf())
    } else if let Some(rest) = written.strip_prefix("~/") {
        Some(home.join(rest))
    } else if written.starts_with('/') {
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
        ConfigError { message, line: None, column: None }
    } else {
        ConfigError { message, line: Some(line), column: Some(column) }
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
    ConfigError { message, line: position.map(|p| p.0), column: position.map(|p| p.1) }
}

/// The byte offset of the JSON string `value` in `text`, from byte `from`.
fn find(text: &str, from: usize, value: &str) -> Option<usize> {
    let quoted = serde_json::to_string(value).ok()?;
    text.get(from..)?.find(&quoted).map(|offset| from + offset)
}
