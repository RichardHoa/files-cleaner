//! Archives, images and installers are never read, so each kind is one
//! Suggested Folder.

pub fn kind(ext: &str) -> &'static str {
    match ext {
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "pages" | "tex" => "Documents",
        "xls" | "xlsx" | "csv" | "tsv" | "ods" | "numbers" => "Spreadsheets",
        "ppt" | "pptx" | "odp" | "key" => "Presentations",
        "png" | "jpg" | "jpeg" | "gif" | "heic" | "heif" | "webp" | "bmp" | "tif" | "tiff"
        | "svg" | "raw" | "cr2" | "nef" | "dng" | "ico" => "Images",
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" | "wmv" => "Videos",
        "mp3" | "m4a" | "wav" | "flac" | "aac" | "ogg" | "aiff" | "opus" => "Audio",
        // macOS, Windows, Linux, Android
        "dmg" | "pkg" | "mpkg" | "exe" | "msi" | "msix" | "msixbundle" | "appx" | "appxbundle"
        | "deb" | "rpm" | "appimage" | "flatpak" | "flatpakref" | "snap" | "run" | "apk"
        | "xapk" | "aab" => "Installers",
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "tbz2" | "xz" | "txz" | "zst"
        | "lz" | "lzma" | "z" | "cab" | "sit" | "sitx" | "iso" => "Archives",
        "epub" | "mobi" | "azw3" => "Books",
        "ttf" | "otf" | "woff" | "woff2" => "Fonts",
        "rs" | "py" | "js" | "mjs" | "ts" | "jsx" | "tsx" | "java" | "kt" | "swift" | "c" | "h"
        | "cc" | "cpp" | "hpp" | "cs" | "go" | "rb" | "php" | "sh" | "ps1" | "sql" | "css"
        | "scss" | "html" | "htm" | "json" | "xml" | "yaml" | "yml" | "toml" | "ipynb"
        | "svelte" | "vue" => "Code",
        "ics" | "vcf" => "Calendar and contacts",
        _ => "Other",
    }
}

#[cfg(test)]
const KINDS: &[&str] = &[
    "Documents",
    "Spreadsheets",
    "Presentations",
    "Images",
    "Videos",
    "Audio",
    "Installers",
    "Archives",
    "Books",
    "Fonts",
    "Code",
    "Calendar and contacts",
    "Other",
];

/// A kind of their own: without text there is nothing to group them by.
pub const SCANNED_PDFS: &str = "Scanned PDFs";

const FIXED_KINDS: &[&str] = &["Archives", "Images", "Installers"];

pub fn fixed_kind(ext: &str) -> Option<&'static str> {
    Some(kind(ext)).filter(|k| FIXED_KINDS.contains(k))
}

/// Looser than equality, so "Images" goes into `My images/` or `Image`.
pub fn matching_folder<'a>(kind: &str, folder_names: &'a [String]) -> Option<&'a str> {
    let stem = |w: &str| w.to_lowercase().trim_end_matches('s').to_string();
    let wanted = stem(kind);
    folder_names
        .iter()
        .find(|name| {
            name.split(|c: char| !c.is_alphanumeric())
                .any(|w| stem(w) == wanted)
        })
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::KINDS;

    fn quoted(text: &str) -> Vec<&str> {
        text.split('"').skip(1).step_by(2).collect()
    }

    #[test]
    fn every_kind_is_listed_here_and_on_the_rules_screen() {
        let source = include_str!("kinds.rs");
        let code = &source[..source.find("#[cfg(test)]").unwrap()];
        for line in code.lines().filter(|l| l.contains("=> \"")) {
            let named = quoted(line.split("=> ").last().unwrap())[0];
            assert!(KINDS.contains(&named), "{named} isn't in KINDS");
        }

        let screen = include_str!("../../../src/screens/rules/rule-text.js");
        let list = screen.split("export const KINDS = [").nth(1).unwrap();
        assert_eq!(quoted(&list[..list.find("];").unwrap()]), KINDS);
    }
}
