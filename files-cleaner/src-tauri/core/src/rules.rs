//! Which Rule a file matches: an extension matcher and a small glob
//! matcher, first match wins.

use crate::clash::split_extension;
use crate::config::Rule;
use crate::scanner::ScanEntry;

/// The first of `rules` that matches `entry`, a file or a Package. A Rule
/// matches when its extensions (if any) and its name glob (if any) both do.
pub fn first_match<'a>(rules: &'a [Rule], entry: &ScanEntry) -> Option<&'a Rule> {
    rules.iter().find(|rule| matches(rule, entry))
}

fn matches(rule: &Rule, entry: &ScanEntry) -> bool {
    let extension = entry.extension.to_lowercase();
    (rule.extensions.is_empty() || rule.extensions.contains(&extension))
        && rule
            .name_glob
            .as_deref()
            .is_none_or(|glob| glob_matches(glob, &entry.name))
}

/// Whether `name` matches `glob`, ignoring letter case: `*` stands for any
/// run of characters (none included) and `?` for exactly one.
fn glob_matches(glob: &str, name: &str) -> bool {
    let glob: Vec<char> = glob.to_lowercase().chars().collect();
    let name: Vec<char> = name.to_lowercase().chars().collect();
    // Where the last `*` was, and how much of `name` it has taken so far,
    // to go back to when the rest stops matching.
    let (mut g, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        match glob.get(g) {
            Some('*') => {
                star = Some((g, n));
                g += 1;
            }
            Some(&c) if c == '?' || c == name[n] => {
                g += 1;
                n += 1;
            }
            _ => match star {
                Some((star_g, star_n)) => {
                    g = star_g + 1;
                    n = star_n + 1;
                    star = Some((star_g, star_n + 1));
                }
                None => return false,
            },
        }
    }
    glob[g..].iter().all(|&c| c == '*')
}

/// The placeholders a rename template may use.
pub const PLACEHOLDERS: [&str; 3] = ["{name}", "{ext}", "{modified}"];

/// The new name for `entry` from the rename `template`: `{name}` is its
/// name without the extension, `{ext}` the extension without the dot, and
/// `{modified}` its modified date as YYYY-MM-DD (UTC). The template gives
/// the part before the extension, which is always kept.
pub fn rename(template: &str, entry: &ScanEntry) -> String {
    let (stem, extension) = split_extension(&entry.name);
    let renamed = template
        .replace("{name}", stem)
        .replace("{ext}", extension.trim_start_matches('.'))
        .replace("{modified}", &date(entry.modified));
    format!("{renamed}{extension}")
}

/// `millis` since the Unix epoch as a UTC date, YYYY-MM-DD.
fn date(millis: i64) -> String {
    let days = millis.div_euclid(86_400_000);
    // Howard Hinnant's days-to-civil: counts in 400-year eras from 0000-03-01.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
