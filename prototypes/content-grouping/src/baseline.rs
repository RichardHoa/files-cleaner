//! Grouper A, no ML: by file kind, then large kinds split by a shared name
//! token, then what's left of a large kind split by modified month.

use crate::keywords::{name_words, title_case, tokens};
use crate::scan::Item;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::SystemTime;

/// A Suggested Folder from the baseline.
pub struct KindFolder {
    pub name: String,
    /// Lowercased words a matching existing subfolder's name could contain,
    /// most specific first.
    pub match_words: Vec<String>,
    pub members: Vec<usize>,
}

pub struct Baseline {
    pub folders: Vec<KindFolder>,
    pub ungrouped: Vec<usize>,
}

pub fn kind(ext: &str) -> &'static str {
    match ext {
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "pages" | "tex" => "Documents",
        "xls" | "xlsx" | "csv" | "tsv" | "ods" | "numbers" => "Spreadsheets",
        "ppt" | "pptx" | "odp" | "key" => "Presentations",
        "png" | "jpg" | "jpeg" | "gif" | "heic" | "heif" | "webp" | "bmp" | "tif" | "tiff" | "svg" | "raw"
        | "cr2" | "nef" | "dng" | "ico" => "Images",
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" | "wmv" => "Videos",
        "mp3" | "m4a" | "wav" | "flac" | "aac" | "ogg" | "aiff" | "opus" => "Audio",
        // macOS, Windows, Linux, Android
        "dmg" | "pkg" | "mpkg" | "exe" | "msi" | "msix" | "msixbundle" | "appx" | "appxbundle" | "deb" | "rpm"
        | "appimage" | "flatpak" | "flatpakref" | "snap" | "run" | "apk" | "xapk" | "aab" => "Installers",
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "tbz2" | "xz" | "txz" | "zst" | "lz" | "lzma" | "z"
        | "cab" | "sit" | "sitx" | "iso" => "Archives",
        "epub" | "mobi" | "azw3" => "Books",
        "ttf" | "otf" | "woff" | "woff2" => "Fonts",
        "rs" | "py" | "js" | "mjs" | "ts" | "jsx" | "tsx" | "java" | "kt" | "swift" | "c" | "h" | "cc" | "cpp"
        | "hpp" | "cs" | "go" | "rb" | "php" | "sh" | "ps1" | "sql" | "css" | "scss" | "html" | "htm" | "json"
        | "xml" | "yaml" | "yml" | "toml" | "ipynb" | "svelte" | "vue" => "Code",
        "ics" | "vcf" => "Calendar and contacts",
        _ => "Other",
    }
}

/// Kinds the content grouper doesn't read: each goes into one folder of its
/// own, whatever the files are called.
pub const FIXED_KINDS: &[&str] = &["Archives", "Images", "Installers"];

/// The file's kind when it is one of the `FIXED_KINDS`.
pub fn fixed_kind(ext: &str) -> Option<&'static str> {
    Some(kind(ext)).filter(|k| FIXED_KINDS.contains(k))
}

fn month(t: SystemTime) -> String {
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (y, m, _) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}")
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian, UTC.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (era * 400 + yoe + if m <= 2 { 1 } else { 0 }, m, d)
}

/// `split_at`: a kind group with at least this many files is split further.
pub fn suggest(items: &[Item], min_size: usize, split_at: usize) -> Baseline {
    let mut by_kind: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        by_kind.entry(kind(&item.ext)).or_default().push(i);
    }
    let mut folders = Vec::new();
    let mut ungrouped = Vec::new();
    for (kind, mut rest) in by_kind {
        let kind_word = kind.to_lowercase();
        if rest.len() >= split_at {
            let name_tokens: HashMap<usize, HashSet<String>> =
                rest.iter().map(|&i| (i, name_words(&items[i].name))).collect();
            loop {
                let mut counts: HashMap<&str, usize> = HashMap::new();
                for i in &rest {
                    for t in &name_tokens[i] {
                        *counts.entry(t.as_str()).or_insert(0) += 1;
                    }
                }
                let Some((token, count)) =
                    counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
                else {
                    break;
                };
                if count < min_size {
                    break;
                }
                let token = token.to_string();
                let (with, without): (Vec<usize>, Vec<usize>) =
                    rest.iter().partition(|i| name_tokens[i].contains(&token));
                folders.push(KindFolder {
                    name: format!("{kind} – {}", title_case(&token)),
                    match_words: vec![token.clone(), kind_word.clone()],
                    members: with,
                });
                rest = without;
            }
            if rest.len() >= split_at {
                let mut by_month: BTreeMap<String, Vec<usize>> = BTreeMap::new();
                for &i in &rest {
                    by_month.entry(month(items[i].modified)).or_default().push(i);
                }
                rest.clear();
                for (month, members) in by_month {
                    if members.len() >= min_size {
                        folders.push(KindFolder {
                            name: format!("{kind} – {month}"),
                            match_words: vec![kind_word.clone()],
                            members,
                        });
                    } else {
                        rest.extend(members);
                    }
                }
            }
        }
        if rest.len() >= min_size {
            rest.sort();
            folders.push(KindFolder { name: kind.to_string(), match_words: vec![kind_word], members: rest });
        } else {
            ungrouped.extend(rest);
        }
    }
    folders.sort_by(|a, b| b.members.len().cmp(&a.members.len()).then(a.name.cmp(&b.name)));
    ungrouped.sort();
    Baseline { folders, ungrouped }
}

/// An existing subfolder whose name contains one of the folder's words
/// (ignoring a plural `s`), trying the most specific word first, so
/// "Documents – Invoice" goes to `Invoices/` before `Documents/`.
pub fn match_by_name<'a>(folder: &KindFolder, folder_names: &'a [String]) -> Option<&'a str> {
    let stem = |w: &str| w.trim_end_matches('s').to_string();
    folder.match_words.iter().map(|w| stem(w)).find_map(|wanted| {
        folder_names.iter().find(|name| tokens(name).iter().any(|w| stem(w) == wanted)).map(String::as_str)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installers_of_every_platform_archives_and_images_are_fixed_kinds() {
        for ext in ["dmg", "exe", "msi", "deb", "appimage", "apk"] {
            assert_eq!(fixed_kind(ext), Some("Installers"), "{ext}");
        }
        assert_eq!(fixed_kind("7z"), Some("Archives"));
        assert_eq!(fixed_kind("heic"), Some("Images"));
        assert_eq!(fixed_kind("pdf"), None);
    }
}
