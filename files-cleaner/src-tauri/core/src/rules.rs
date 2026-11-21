//! The shape of a Rule (ADR 0013), as saved in the config: which Loose
//! Files it matches and how it groups them into Suggested Folders. Applying
//! Rules to files is the grouping crate's job.

use serde::{Deserialize, Serialize};

use crate::folder_name::safe_folder_name;

/// A saved instruction for how Loose Files are sorted into Suggested
/// Folders at every scan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    /// An off Rule is kept but matches nothing.
    #[serde(default = "on")]
    pub enabled: bool,
    /// The user's own words the Rule was drafted from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<String>,
    /// What every matched file has; nothing set matches every Loose File.
    #[serde(default, rename = "match")]
    pub matches: RuleMatch,
    pub group_by: GroupBy,
}

fn on() -> bool {
    true
}

/// What a file must have to match a Rule: every field set, combined.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleMatch {
    /// A file kind as the grouping engine names it, such as "Images".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Without the dot, any of them, ignoring case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<NameMatch>,
    /// Files whose names contain any of these are left out, compared as
    /// `NameMatch` compares names.
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

/// What a file's name must have. Names are compared ignoring case and
/// accents ("Báo cáo" is "bao cao"), and every run of spaces, `_`, `-` and
/// `.` counts as one space, so "baraka trip" fits "Baraka_Trip-2024.jpg".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NameMatch {
    #[serde(default)]
    pub how: NameHow,
    /// Any of them.
    pub words: Vec<String>,
}

/// How a name is held to a Rule's words. The name is taken with or without
/// its extension, whichever fits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NameHow {
    /// Anywhere in the name.
    #[default]
    Contains,
    StartsWith,
    EndsWith,
    /// The whole name.
    Is,
    /// The whole name, where `*` stands for any characters and `?` for one,
    /// as in `Screenshot*`.
    Pattern,
}

/// A file's date within a range of days before today.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DateMatch {
    #[serde(default)]
    pub field: DateField,
    /// Dated at most this many days before today: 0 is today only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within_days: Option<u32>,
    /// Dated more than this many days before today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub older_than_days: Option<u32>,
}

/// Which of a file's dates a Rule goes by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DateField {
    /// When it was last changed.
    #[default]
    Changed,
    /// When it was added to its folder; the date changed where the file
    /// system doesn't record it.
    Added,
}

/// How a Rule groups the files it matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum GroupBy {
    /// All into one folder. `{year}`, `{month}` and `{day}` in its name are
    /// the file's date ("2026", "2026-11", "2026-11-20"), and a `/` puts a
    /// folder inside another: `Screenshots/{month}`.
    Folder {
        name: String,
        #[serde(default)]
        date: DateField,
    },
    /// By the file's date.
    Date {
        #[serde(default)]
        date: DateField,
        by: DateGrouping,
    },
    /// One folder per file kind, named as the grouping engine names kinds.
    Kind,
    /// By what the files are about: each into the most like of `folders`,
    /// or, when there are none, into folders proposed from the files
    /// themselves. Needs the Grouping Models.
    Meaning {
        #[serde(default)]
        folders: Vec<MeaningFolder>,
    },
}

/// A folder the user named for a Rule that groups by meaning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeaningFolder {
    pub name: String,
    /// What belongs in it, in the user's words, compared along with its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DateGrouping {
    /// Today, Yesterday, This week, This month and Older.
    Buckets,
    /// One folder per month, such as "2026-11".
    Month,
    /// One folder per year, such as "2026".
    Year,
}

/// Why `rule` can't be saved, if it can't.
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
