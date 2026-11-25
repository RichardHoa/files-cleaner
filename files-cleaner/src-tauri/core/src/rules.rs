//! Only the shape (ADR 0013): applying Rules is the grouping crate's job, as
//! some need the models.

use serde::{Deserialize, Serialize};

use crate::folder_name::safe_folder_name;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    /// An off Rule is kept but matches nothing.
    #[serde(default = "on")]
    pub enabled: bool,
    /// No longer used; kept so older configs still load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<String>,
    /// Empty matches every Loose File.
    #[serde(default, rename = "match")]
    pub matches: RuleMatch,
    pub group_by: GroupBy,
}

fn on() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleMatch {
    /// As the grouping engine names it, such as "Images".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Without the dot, ignoring case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<NameMatch>,
    /// Takes precedence over what `name` matches.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub except_names: Vec<String>,
    /// Bytes, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_size: Option<u64>,
    /// Bytes, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<DateMatch>,
}

/// Case, accents and separators are ignored, so "baraka trip" fits
/// "Baraka_Trip-2024.jpg".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NameMatch {
    #[serde(default)]
    pub how: NameHow,
    pub words: Vec<String>,
}

/// With or without the extension, whichever fits, as users write either.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NameHow {
    #[default]
    Contains,
    StartsWith,
    EndsWith,
    Is,
    /// `*` is any characters and `?` one, as in `Screenshot*`.
    Pattern,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DateMatch {
    #[serde(default)]
    pub field: DateField,
    /// 0 is today only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub older_than_days: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DateField {
    #[default]
    Changed,
    /// Falls back on the date changed where the file system doesn't record it.
    Added,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum GroupBy {
    /// `{year}`, `{month}` and `{day}` in the name are the file's date; a `/`
    /// nests folders.
    Folder {
        name: String,
        #[serde(default)]
        date: DateField,
    },
    Date {
        #[serde(default)]
        date: DateField,
        by: DateGrouping,
    },
    Kind,
    /// No `folders` means they are proposed from the files. Needs the Grouping
    /// Models.
    Meaning {
        #[serde(default)]
        folders: Vec<MeaningFolder>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeaningFolder {
    pub name: String,
    /// In the user's words; compared along with the name, which may say too
    /// little.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DateGrouping {
    Buckets,
    Month,
    Year,
}

pub fn rule_problem(rule: &Rule) -> Option<String> {
    let m = &rule.matches;
    if let (Some(min), Some(max)) = (m.min_size, m.max_size) {
        if min > max {
            return Some("a Rule's smallest size must not be above its largest".into());
        }
    }
    if m.extensions
        .iter()
        .any(|e| e.trim_start_matches('.').trim().is_empty())
    {
        return Some("a Rule's extensions must not be empty".into());
    }
    if m.name
        .as_ref()
        .is_some_and(|n| n.words.is_empty() || n.words.iter().any(|w| w.trim().is_empty()))
        || m.except_names.iter().any(|w| w.trim().is_empty())
    {
        return Some("a Rule's name words must not be empty".into());
    }
    if m.date
        .as_ref()
        .is_some_and(|d| d.within_days.is_none() && d.older_than_days.is_none())
    {
        return Some("a Rule's date needs a number of days".into());
    }
    if let GroupBy::Folder { name, .. } = &rule.group_by {
        let sample = name
            .replace("{year}", "2026")
            .replace("{month}", "2026-11")
            .replace("{day}", "2026-11-20");
        if sample
            .split('/')
            .any(|part| safe_folder_name(part).is_none())
        {
            return Some(format!("\u{201c}{name}\u{201d} can't be a folder name"));
        }
    }
    if let GroupBy::Meaning { folders } = &rule.group_by {
        let mut seen = std::collections::HashSet::new();
        for folder in folders {
            let name = folder.name.trim();
            if safe_folder_name(name).is_none() {
                return Some(format!("\u{201c}{name}\u{201d} can't be a folder name"));
            }
            if !seen.insert(name.to_lowercase()) {
                return Some(format!("\u{201c}{name}\u{201d} is named twice"));
            }
        }
    }
    None
}
