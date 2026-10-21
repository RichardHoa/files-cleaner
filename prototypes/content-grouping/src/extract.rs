//! Reading a short text for each file: its name, plus what's inside it when
//! the format is one we can read without OCR. Files are only ever opened for
//! reading.

use crate::download;
use crate::scan::Item;
use pdfium_render::prelude::Pdfium;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Method {
    PdfText,
    OfficeXml,
    PlainText,
    NameOnly,
}

impl Method {
    pub fn label(self) -> &'static str {
        match self {
            Method::PdfText => "PDF text",
            Method::OfficeXml => "Office XML",
            Method::PlainText => "plain text",
            Method::NameOnly => "name only",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Content {
    pub method: Method,
    /// The file name as words.
    pub name: String,
    /// What's inside, at most `max_chars`; empty when read by name only.
    pub body: String,
}

impl Content {
    pub fn name_only(name: String) -> Self {
        Content { method: Method::NameOnly, name, body: String::new() }
    }
}

pub struct Limits {
    pub max_chars: usize,
    pub pdf_timeout: Duration,
    /// The PDFium library the PDF child loads.
    pub pdfium: PathBuf,
}

const PLAIN_TEXT: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "xml", "html", "htm", "log", "yaml", "yml", "toml", "ini",
    "tex", "rst", "org", "srt", "vtt", "rs", "py", "js", "mjs", "ts", "jsx", "tsx", "java", "kt", "swift", "c",
    "h", "cc", "cpp", "hpp", "cs", "go", "rb", "php", "sh", "zsh", "bash", "ps1", "sql", "css", "scss", "r",
    "m", "lua", "dart", "scala", "svelte", "vue", "ipynb",
];

/// Most bytes read from one part of an Office file, so a zip bomb can't
/// exhaust memory.
const MAX_XML_BYTES: u64 = 4 * 1024 * 1024;

pub fn read(item: &Item, limits: &Limits) -> Content {
    let (path, ext) = (item.path.as_path(), item.ext.as_str());
    let inside = match ext {
        "pdf" => pdf_text(&limits.pdfium, path, limits.pdf_timeout, limits.max_chars).map(|t| (Method::PdfText, t)),
        "docx" | "pptx" | "xlsx" => office_text(path, ext, limits.max_chars).map(|t| (Method::OfficeXml, t)),
        _ if PLAIN_TEXT.contains(&ext) => plain_text(path, limits.max_chars).map(|t| (Method::PlainText, t)),
        _ => None,
    };
    let name = name_words(&item.name);
    match inside.map(|(m, t)| (m, truncate(&collapse_whitespace(&t), limits.max_chars))) {
        Some((method, body)) if body.chars().filter(|c| c.is_alphabetic()).count() >= 20 => {
            Content { method, name, body }
        }
        _ => Content::name_only(name),
    }
}

/// The file name without its extension, with separators and camelCase
/// turned into spaces: `Screenshot_2026-10-03 at 14.22.01.png` becomes
/// `Screenshot 2026 10 03 at 14 22 01`.
pub fn name_words(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let mut out = String::with_capacity(stem.len() + 8);
    let mut prev: Option<char> = None;
    for c in stem.chars() {
        if c.is_alphanumeric() {
            if let Some(p) = prev {
                let camel = p.is_lowercase() && c.is_uppercase();
                let digit_edge = p.is_alphabetic() != c.is_alphabetic() && (p.is_numeric() || c.is_numeric());
                if (camel || digit_edge) && !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            out.push(c);
        } else if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        prev = Some(c);
    }
    out.trim_end().to_string()
}

/// Runs of spaces become one space; line breaks stay (one per run), since
/// keyword phrases never span them.
fn collapse_whitespace(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((i, _)) => text[..i].to_string(),
        None => text.to_string(),
    }
}

/// Pages of a PDF read for its text; the rest of the document is never decoded.
const PDF_PAGES: i32 = 3;
/// The pdfium-binaries release downloaded by default. It must match the
/// `pdfium_7881` feature of pdfium-render in Cargo.toml.
pub const PDFIUM_RELEASE: &str = "chromium/7881";

/// The library file of the default PDFium, downloaded once from
/// bblanchon/pdfium-binaries into the user cache folder (about 6 MB).
pub fn ensure_pdfium() -> Result<PathBuf, String> {
    let dir = download::cache_root()
        .ok_or("no user cache folder; pass --pdfium")?
        .join(PDFIUM_RELEASE.replace('/', "-"));
    // The archives keep the Windows DLL in bin/, the others in lib/.
    let lib = Pdfium::pdfium_platform_library_name_at_path(&dir.join(if cfg!(windows) { "bin" } else { "lib" }));
    if lib.exists() {
        return Ok(lib);
    }
    let os = match std::env::consts::OS {
        "macos" => "mac",
        "windows" => "win",
        "linux" => "linux",
        other => return Err(format!("no prebuilt PDFium for {other}; pass --pdfium")),
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => return Err(format!("no prebuilt PDFium for {other}; pass --pdfium")),
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    let archive = dir.join("pdfium.tgz");
    let release = PDFIUM_RELEASE.replace('/', "%2F");
    download::fetch(
        &format!("https://github.com/bblanchon/pdfium-binaries/releases/download/{release}/pdfium-{os}-{arch}.tgz"),
        &archive,
    )?;
    // `tar` ships with macOS, Linux and Windows 10+.
    let status = Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .status()
        .map_err(|e| format!("can't run tar to unpack {} ({e})", archive.display()))?;
    let _ = std::fs::remove_file(&archive);
    if !status.success() || !lib.exists() {
        return Err(format!("unpacking {} didn't give {}", archive.display(), lib.display()));
    }
    Ok(lib)
}
/// How often the parent checks whether the PDF child has finished.
const PDF_POLL: Duration = Duration::from_millis(20);

/// The PDF's text layer. PDFium is C++ and can crash or take long on odd
/// files, so it runs in a child process of this same program
/// (`--extract-pdf`), which is killed after `timeout`; the OS then frees
/// everything it held.
fn pdf_text(pdfium: &Path, path: &Path, timeout: Duration, max_chars: usize) -> Option<String> {
    let mut child = Command::new(std::env::current_exe().ok()?)
        .arg(PDF_CHILD_FLAG)
        .arg(pdfium)
        .arg(path)
        .arg(max_chars.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    // The child prints at most `max_chars` characters, well under a pipe's
    // buffer, so it never blocks on a full pipe while we wait.
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(PDF_POLL),
        }
    }
    let mut text = String::new();
    child.stdout.take()?.read_to_string(&mut text).ok()?;
    Some(text)
}

/// The flag `main` checks for before parsing arguments: run as the PDF child.
pub const PDF_CHILD_FLAG: &str = "--extract-pdf";

/// The PDF child's work: the text of the first `PDF_PAGES` pages of `path`,
/// at most `max_chars` characters, printed to stdout. `pdfium` is the
/// library file to load.
pub fn pdf_child(pdfium: &Path, path: &Path, max_chars: usize) -> Result<(), String> {
    let pdfium = Pdfium::new(Pdfium::bind_to_library(pdfium).map_err(|e| e.to_string())?);
    // Opened through a seekable reader, not read into memory: PDFium reads
    // the cross-reference table and then only the objects of the pages it
    // is asked for, so a 2 GB PDF costs about what a 2 MB one does.
    let doc = pdfium.load_pdf_from_file(path, None).map_err(|e| e.to_string())?;
    let pages = doc.pages();
    let mut text = String::new();
    for index in 0..pages.len().min(PDF_PAGES) {
        let page = pages.get(index).map_err(|e| e.to_string())?;
        text.push_str(&page.text().map_err(|e| e.to_string())?.all());
        text.push('\n');
        if text.chars().count() >= max_chars {
            break;
        }
    }
    print!("{}", truncate(&text, max_chars));
    Ok(())
}

fn plain_text(path: &Path, max_chars: usize) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(max_chars as u64 * 4).read_to_end(&mut bytes).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn office_text(path: &Path, ext: &str, max_chars: usize) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut parts: Vec<String> = match ext {
        "docx" => vec!["word/document.xml".into()],
        "xlsx" => vec!["xl/sharedStrings.xml".into()],
        _ => archive
            .file_names()
            .filter_map(Result::ok)
            .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
            .map(|n| n.into_owned())
            .collect(),
    };
    // slide2 before slide10
    parts.sort_by_key(|n| n.trim_start_matches(|c: char| !c.is_ascii_digit()).trim_end_matches(".xml").parse::<u32>().unwrap_or(0));
    let mut text = String::new();
    for part in parts {
        let Ok(entry) = archive.by_name(&part) else { continue };
        let mut xml = String::new();
        if entry.take(MAX_XML_BYTES).read_to_string(&mut xml).is_err() {
            continue;
        }
        strip_xml(&xml, &mut text, max_chars);
        if text.chars().count() >= max_chars {
            break;
        }
        text.push(' ');
    }
    Some(text)
}

/// Appends the text nodes of an Office XML part to `out`. Runs inside one
/// paragraph join directly (Word splits words across runs); the ends of
/// paragraphs, cells, shared strings and line breaks become spaces.
fn strip_xml(xml: &str, out: &mut String, max_chars: usize) {
    let mut rest = xml;
    let mut chars = out.chars().count();
    while let Some(lt) = rest.find('<') {
        let text = decode_entities(&rest[..lt]);
        chars += text.chars().count();
        out.push_str(&text);
        if chars >= max_chars {
            return;
        }
        let Some(gt) = rest[lt..].find('>') else { return };
        let tag = &rest[lt + 1..lt + gt];
        let local = tag.trim_start_matches('/').split([' ', '/']).next().unwrap_or("");
        let local = local.rsplit(':').next().unwrap_or(local);
        if matches!(local, "p" | "br" | "tab" | "si" | "c" | "tc" | "cr") && !out.ends_with(' ') {
            out.push(' ');
            chars += 1;
        }
        rest = &rest[lt + gt + 1..];
    }
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}
