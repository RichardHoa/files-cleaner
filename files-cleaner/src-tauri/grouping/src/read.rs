//! Reading a short text for each file: its name, plus what's inside it when
//! the format is one we can read without OCR. Files are only ever opened for
//! reading.

use pdfium_render::prelude::Pdfium;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What the models are given about one file.
#[derive(Debug, Clone)]
pub struct Content {
    /// The file name as words.
    pub name: String,
    /// What's inside, at most `MAX_CHARS`; empty when read by name only.
    pub body: String,
}

/// Most characters of content read from each file.
const MAX_CHARS: usize = 2000;
/// A PDF whose text isn't out by then is read by its name.
const PDF_TIMEOUT: Duration = Duration::from_secs(10);
/// Pages of a PDF read for its text; the rest of the document is never decoded.
const PDF_PAGES: i32 = 3;

const PLAIN_TEXT: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "xml", "html", "htm", "log", "yaml", "yml", "toml", "ini",
    "tex", "rst", "org", "srt", "vtt", "rs", "py", "js", "mjs", "ts", "jsx", "tsx", "java", "kt", "swift", "c",
    "h", "cc", "cpp", "hpp", "cs", "go", "rb", "php", "sh", "zsh", "bash", "ps1", "sql", "css", "scss", "r",
    "m", "lua", "dart", "scala", "svelte", "vue", "ipynb",
];

/// Most bytes read from one part of an Office file, so a zip bomb can't
/// exhaust memory.
const MAX_XML_BYTES: u64 = 4 * 1024 * 1024;

/// Reads the file at `path` named `name`. `pdfium` is the PDFium library;
/// without it, PDFs are read by name.
pub fn read(path: &Path, name: &str, pdfium: Option<&Path>) -> Content {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    let inside = match ext.as_str() {
        "pdf" => pdfium.and_then(|pdfium| pdf_text(pdfium, path)),
        "docx" | "pptx" | "xlsx" => office_text(path, &ext),
        _ if PLAIN_TEXT.contains(&ext.as_str()) => plain_text(path),
        _ => None,
    };
    let name = name_words(name);
    match inside.map(|t| truncate(&collapse_whitespace(&t), MAX_CHARS)) {
        Some(body) if body.chars().filter(|c| c.is_alphabetic()).count() >= 20 => Content { name, body },
        _ => Content { name, body: String::new() },
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

/// Runs of spaces become one space; line breaks stay, one per run.
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

/// How often the parent checks whether the PDF child has finished.
const PDF_POLL: Duration = Duration::from_millis(20);

/// The flag the app is started with to read one PDF: see `pdf_child_main`.
const PDF_CHILD_FLAG: &str = "--read-pdf";

/// The PDF's text layer. PDFium is C++ and can crash or take long on odd
/// files, so it runs in a child process of the app itself (`PDF_CHILD_FLAG`),
/// which is killed after `PDF_TIMEOUT`; the OS then frees everything it held.
fn pdf_text(pdfium: &Path, path: &Path) -> Option<String> {
    let mut child = Command::new(std::env::current_exe().ok()?)
        .arg(PDF_CHILD_FLAG)
        .arg(pdfium)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    // The child prints at most `MAX_CHARS` characters, well under a pipe's
    // buffer, so it never blocks on a full pipe while we wait.
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if started.elapsed() >= PDF_TIMEOUT => {
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

/// When the app was started to read one PDF, reads it and says how the
/// process should exit; otherwise `None`, and the app starts as usual. The
/// app's `main` calls this first. The text of the first `PDF_PAGES` pages,
/// at most `MAX_CHARS` characters, goes to stdout.
pub fn pdf_child_main() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != PDF_CHILD_FLAG) {
        return None;
    }
    let (Some(pdfium), Some(path)) = (args.get(2), args.get(3)) else { return Some(1) };
    // A panic or a crash in PDFium just ends this process; the parent sees a failure.
    Some(match pdf_child(Path::new(pdfium), Path::new(path)) {
        Ok(()) => 0,
        Err(_) => 1,
    })
}

fn pdf_child(pdfium: &Path, path: &Path) -> Result<(), String> {
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
        if text.chars().count() >= MAX_CHARS {
            break;
        }
    }
    print!("{}", truncate(&text, MAX_CHARS));
    Ok(())
}

fn plain_text(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(MAX_CHARS as u64 * 4).read_to_end(&mut bytes).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn office_text(path: &Path, ext: &str) -> Option<String> {
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
    parts.sort_by_key(|n| {
        n.trim_start_matches(|c: char| !c.is_ascii_digit()).trim_end_matches(".xml").parse::<u32>().unwrap_or(0)
    });
    let mut text = String::new();
    for part in parts {
        let Ok(entry) = archive.by_name(&part) else { continue };
        let mut xml = String::new();
        if entry.take(MAX_XML_BYTES).read_to_string(&mut xml).is_err() {
            continue;
        }
        strip_xml(&xml, &mut text);
        if text.chars().count() >= MAX_CHARS {
            break;
        }
        text.push(' ');
    }
    Some(text)
}

/// Appends the text nodes of an Office XML part to `out`. Runs inside one
/// paragraph join directly (Word splits words across runs); the ends of
/// paragraphs, cells, shared strings and line breaks become spaces.
fn strip_xml(xml: &str, out: &mut String) {
    let mut rest = xml;
    let mut chars = out.chars().count();
    while let Some(lt) = rest.find('<') {
        let text = decode_entities(&rest[..lt]);
        chars += text.chars().count();
        out.push_str(&text);
        if chars >= MAX_CHARS {
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
