//! Rules applied to Loose Files by code alone (ADR 0013): dates, kinds,
//! extensions, names and sizes are checked here, never by a model. Each
//! file goes where the first enabled Rule that matches it says, even when
//! a later one matches it too (ADR 0014). A Rule by meaning only takes its
//! files here; the models sort them.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, NaiveDate, TimeZone};
use files_cleaner_core::{
    DateField, DateGrouping, DateMatch, GroupBy, NameHow, NameMatch, Rule, RuleMatch, ScanEntry,
    ScanResult,
};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::{kinds, Basis, SuggestedFolder};

/// Whether `entry` is a Loose File: directly in the Scan Root, with an
/// extension, and not a Package.
pub fn is_loose(entry: &ScanEntry) -> bool {
    entry.relative_path.len() == 1 && !entry.is_package && !entry.extension.is_empty()
}

/// The Loose Files a Rule groups by meaning, which the models sort.
pub struct ByMeaning {
    pub rule: Rule,
    pub files: Vec<PathBuf>,
}

/// The Suggested Folders `rules` make of the Loose Files of `scan`, dates
/// counted from `today`, in the order they were first filled, and the files
/// each Rule by meaning takes, in the Rules' order. Folders of the same name
/// are one, made by the first Rule that filled it. Rules by meaning take
/// only files that are read, and only `by_meaning`: otherwise their files
/// go by the next Rule they fit.
pub fn apply(
    scan: &ScanResult,
    rules: &[Rule],
    today: NaiveDate,
    by_meaning: bool,
) -> (Vec<SuggestedFolder>, Vec<ByMeaning>) {
    let mut folders: Vec<SuggestedFolder> = Vec::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    let mut taken: BTreeMap<usize, Vec<PathBuf>> = BTreeMap::new();
    for entry in scan.entries.iter().filter(|e| is_loose(e)) {
        let read = kinds::fixed_kind(&entry.extension.to_lowercase()).is_none();
        let Some((r, rule)) = rules.iter().enumerate().find(|(_, rule)| {
            rule.enabled
                && (!is_by_meaning(rule) || (by_meaning && read))
                && matches(rule, entry, today)
        }) else {
            continue;
        };
        if is_by_meaning(rule) {
            taken.entry(r).or_default().push(entry.path.clone());
            continue;
        }
        let name = folder_name(rule, entry, today);
        let i = *by_name.entry(name.to_lowercase()).or_insert_with(|| {
            folders.push(SuggestedFolder {
                existing: existing_folder(&scan.root, &name),
                name,
                files: Vec::new(),
                basis: Basis::Rule,
                rule: Some(rule.clone()),
            });
            folders.len() - 1
        });
        folders[i].files.push(entry.path.clone());
    }
    let taken = taken
        .into_iter()
        .map(|(r, files)| ByMeaning {
            rule: rules[r].clone(),
            files,
        })
        .collect();
    (folders, taken)
}

fn is_by_meaning(rule: &Rule) -> bool {
    matches!(rule.group_by, GroupBy::Meaning { .. })
}

/// For each of `rules`, the first enabled Rule before it that some file
/// could match too, if it's enabled itself: such a file goes where that
/// earlier Rule says. Two Rules are kept apart only where a field rules
/// out every file one of them matches, so this can warn of an overlap no
/// file has, never miss one.
pub fn overlaps(rules: &[Rule]) -> Vec<Option<usize>> {
    rules
        .iter()
        .enumerate()
        .map(|(i, rule)| {
            if !rule.enabled {
                return None;
            }
            rules[..i]
                .iter()
                .position(|earlier| earlier.enabled && may_overlap(&earlier.matches, &rule.matches))
        })
        .collect()
}

fn may_overlap(a: &RuleMatch, b: &RuleMatch) -> bool {
    kinds_overlap(a, b)
        && a.min_size.unwrap_or(0) <= b.max_size.unwrap_or(u64::MAX)
        && b.min_size.unwrap_or(0) <= a.max_size.unwrap_or(u64::MAX)
        && dates_overlap(a.date.as_ref(), b.date.as_ref())
        && !names_apart(a, b)
}

fn kinds_overlap(a: &RuleMatch, b: &RuleMatch) -> bool {
    let (kind_a, kind_b) = (
        a.kind.as_deref().map(str::trim),
        b.kind.as_deref().map(str::trim),
    );
    match (extensions_of(a), extensions_of(b)) {
        (Some(x), Some(y)) => x.iter().any(|e| y.contains(e)),
        (Some(x), None) => {
            kind_b.is_none_or(|k| x.iter().any(|e| kinds::kind(e).eq_ignore_ascii_case(k)))
        }
        (None, Some(y)) => {
            kind_a.is_none_or(|k| y.iter().any(|e| kinds::kind(e).eq_ignore_ascii_case(k)))
        }
        (None, None) => match (kind_a, kind_b) {
            (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
            _ => true,
        },
    }
}

/// The extensions `m` matches, lowercase and without the dot, if it names
/// any: those of its kind only, when it has one.
fn extensions_of(m: &RuleMatch) -> Option<Vec<String>> {
    if m.extensions.is_empty() {
        return None;
    }
    Some(
        m.extensions
            .iter()
            .map(|e| e.trim().trim_start_matches('.').to_lowercase())
            .filter(|e| {
                m.kind
                    .as_ref()
                    .is_none_or(|kind| kinds::kind(e).eq_ignore_ascii_case(kind.trim()))
            })
            .collect(),
    )
}

/// Whether some day before today fits both; dates of different fields
/// always may.
fn dates_overlap(a: Option<&DateMatch>, b: Option<&DateMatch>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return true;
    };
    // Days before today, inclusive; a date after today is below zero.
    let days = |d: &DateMatch| {
        (
            d.older_than_days.map_or(i64::MIN, |n| i64::from(n) + 1),
            d.within_days.map_or(i64::MAX, i64::from),
        )
    };
    let ((a_low, a_high), (b_low, b_high)) = (days(a), days(b));
    a.field != b.field || (a_low <= b_high && b_low <= a_high)
}

/// Whether no file name fits both `a` and `b`.
fn names_apart(a: &RuleMatch, b: &RuleMatch) -> bool {
    left_out(a, b)
        || left_out(b, a)
        || a.name.as_ref().zip(b.name.as_ref()).is_some_and(|(x, y)| {
            x.words.iter().all(|w| {
                y.words
                    .iter()
                    .all(|v| words_apart(x.how, &fold(w), y.how, &fold(v)))
            })
        })
}

/// Whether every name `a` matches contains a word `b` leaves out.
fn left_out(a: &RuleMatch, b: &RuleMatch) -> bool {
    a.name.as_ref().is_some_and(|name| {
        name.how != NameHow::Pattern
            && name.words.iter().all(|w| {
                let w = fold(w);
                b.except_names
                    .iter()
                    .map(|x| fold(x))
                    .any(|x| !x.is_empty() && w.contains(&x))
            })
    })
}

/// Whether no name fits both folded words, `a` held as `a_how` and `b` as
/// `b_how`. A name fits as itself or as its stem, which begins it before a
/// space, so starts and whole names are apart only when neither word could
/// begin the other; anything else may always overlap.
fn words_apart(a_how: NameHow, a: &str, b_how: NameHow, b: &str) -> bool {
    match (a_how, b_how) {
        (NameHow::Is, NameHow::Is) => {
            a != b && !a.starts_with(&format!("{b} ")) && !b.starts_with(&format!("{a} "))
        }
        (NameHow::StartsWith | NameHow::Is, NameHow::StartsWith | NameHow::Is) => {
            !a.starts_with(b) && !b.starts_with(a)
        }
        _ => false,
    }
}

fn matches(rule: &Rule, entry: &ScanEntry, today: NaiveDate) -> bool {
    let m = &rule.matches;
    let extension = entry.extension.to_lowercase();
    m.kind
        .as_ref()
        .is_none_or(|kind| kinds::kind(&extension).eq_ignore_ascii_case(kind.trim()))
        && (m.extensions.is_empty()
            || m.extensions.iter().any(|e| {
                e.trim()
                    .trim_start_matches('.')
                    .eq_ignore_ascii_case(&extension)
            }))
        && m.name
            .as_ref()
            .is_none_or(|name| name_fits(name, &entry.name))
        && !m.except_names.iter().any(|word| {
            let word = fold(word);
            !word.is_empty() && fold(&entry.name).contains(&word)
        })
        && m.min_size.is_none_or(|min| entry.size >= min)
        && m.max_size.is_none_or(|max| entry.size <= max)
        && m.date.as_ref().is_none_or(|d| {
            let days = days_before(today, date_of(entry, d.field));
            d.within_days.is_none_or(|n| days <= i64::from(n))
                && d.older_than_days.is_none_or(|n| days > i64::from(n))
        })
}

/// The Suggested Folder `rule` puts `entry` in.
fn folder_name(rule: &Rule, entry: &ScanEntry, today: NaiveDate) -> String {
    match &rule.group_by {
        GroupBy::Folder { name, date } => {
            let date = date_of(entry, *date);
            name.replace("{year}", &date.format("%Y").to_string())
                .replace("{month}", &date.format("%Y-%m").to_string())
                .replace("{day}", &date.format("%Y-%m-%d").to_string())
        }
        GroupBy::Date { date, by } => {
            let date = date_of(entry, *date);
            match by {
                DateGrouping::Buckets => bucket(today, date).to_string(),
                DateGrouping::Month => date.format("%Y-%m").to_string(),
                DateGrouping::Year => date.format("%Y").to_string(),
            }
        }
        GroupBy::Kind => kinds::kind(&entry.extension.to_lowercase()).to_string(),
        GroupBy::Meaning { .. } => unreachable!("a Rule by meaning names no folder by code"),
    }
}

/// Today, Yesterday, This week (from Monday), This month or Older; a date
/// after today is today.
fn bucket(today: NaiveDate, date: NaiveDate) -> &'static str {
    let days = days_before(today, date);
    let monday = today - chrono::Days::new(u64::from(today.weekday().num_days_from_monday()));
    if days <= 0 {
        "Today"
    } else if days == 1 {
        "Yesterday"
    } else if date >= monday {
        "This week"
    } else if (date.year(), date.month()) == (today.year(), today.month()) {
        "This month"
    } else {
        "Older"
    }
}

/// The local date of `entry`'s `field`; the date changed where the date
/// added isn't recorded.
fn date_of(entry: &ScanEntry, field: DateField) -> NaiveDate {
    let millis = match field {
        DateField::Changed => entry.modified,
        DateField::Added => entry.created.unwrap_or(entry.modified),
    };
    Local
        .timestamp_millis_opt(millis)
        .single()
        .map(|t| t.date_naive())
        .unwrap_or_default()
}

fn days_before(today: NaiveDate, date: NaiveDate) -> i64 {
    (today - date).num_days()
}

/// Whether the file name `name`, with or without its extension, fits any
/// of `m`'s words.
fn name_fits(m: &NameMatch, name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .map_or(name.into(), |s| s.to_string_lossy());
    let names = [fold(name), fold(&stem)];
    m.words.iter().map(|w| fold(w)).any(|word| {
        !word.is_empty()
            && names.iter().any(|name| match m.how {
                NameHow::Contains => name.contains(&word),
                NameHow::StartsWith => name.starts_with(&word),
                NameHow::EndsWith => name.ends_with(&word),
                NameHow::Is => *name == word,
                NameHow::Pattern => pattern_matches(&word, name),
            })
    })
}

/// `text` as names are compared: lowercase, without accents ("đ" is "d"),
/// and each run of spaces, `_`, `-` and `.` one space, trimmed.
fn fold(text: &str) -> String {
    let mut folded = String::new();
    for c in text.nfd().filter(|&c| !is_combining_mark(c)) {
        if c.is_whitespace() || matches!(c, '_' | '-' | '.') {
            if !folded.is_empty() && !folded.ends_with(' ') {
                folded.push(' ');
            }
        } else if matches!(c, 'đ' | 'Đ') {
            folded.push('d');
        } else {
            folded.extend(c.to_lowercase());
        }
    }
    folded.truncate(folded.trim_end().len());
    folded
}

/// Whether `name` is all of `pattern`, both folded: `*` is any run of
/// characters and `?` any one.
fn pattern_matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    // `fits[j]`: the pattern so far matches the first `j` characters of the name.
    let mut fits = vec![false; name.len() + 1];
    fits[0] = true;
    for p in pattern {
        let mut next = vec![false; name.len() + 1];
        for j in 0..=name.len() {
            next[j] = match p {
                '*' => fits[j] || (j > 0 && next[j - 1]),
                '?' => j > 0 && fits[j - 1],
                c => j > 0 && fits[j - 1] && name[j - 1] == c,
            };
        }
        fits = next;
    }
    fits[name.len()]
}

/// The folder at `name` below `root`, each `/`-separated part matched
/// ignoring case, if it's there.
pub fn existing_folder(root: &Path, name: &str) -> Option<PathBuf> {
    name.split('/')
        .try_fold(root.to_path_buf(), |parent, part| {
            let wanted = part.trim().to_lowercase();
            fs::read_dir(&parent)
                .ok()?
                .flatten()
                .find(|child| {
                    child.file_name().to_string_lossy().to_lowercase() == wanted
                        && child.file_type().is_ok_and(|t| t.is_dir())
                })
                .map(|child| child.path())
        })
}
