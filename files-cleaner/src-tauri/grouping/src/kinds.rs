//! File kinds by extension. Archives, images and installers are never read:
//! each of those kinds becomes one Suggested Folder of its own.

/// The kind of a file with extension `ext` (lowercased, without the dot),
/// named as its folder would be.
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

/// Kinds that are never read: each goes into one folder of its own,
/// whatever the files are called.
const FIXED_KINDS: &[&str] = &["Archives", "Images", "Installers"];

/// The file's kind when it is one of the `FIXED_KINDS`.
pub fn fixed_kind(ext: &str) -> Option<&'static str> {
    Some(kind(ext)).filter(|k| FIXED_KINDS.contains(k))
}

/// The first of `folder_names` with a word that is `kind`'s, ignoring case
/// and a plural `s`: "Images" goes into `My images/` or `Image`.
pub fn matching_folder<'a>(kind: &str, folder_names: &'a [String]) -> Option<&'a str> {
    let stem = |w: &str| w.to_lowercase().trim_end_matches('s').to_string();
    let wanted = stem(kind);
    folder_names
        .iter()
        .find(|name| name.split(|c: char| !c.is_alphanumeric()).any(|w| stem(w) == wanted))
        .map(String::as_str)
}
