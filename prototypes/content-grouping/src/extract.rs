//! Reading a short text for each file: its name, plus what's inside it when
//! the format is one we can read without OCR. Files are only ever opened for
//! reading.

use crate::scan::Item;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

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
    /// The name as words, then the content, at most `max_chars` of content.
    pub text: String,
}

pub struct Limits {
    pub max_chars: usize,
    pub pdf_timeout: Duration,
    pub max_pdf_bytes: u64,
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
        "pdf" if item.size <= limits.max_pdf_bytes => pdf_text(path, limits.pdf_timeout).map(|t| (Method::PdfText, t)),
        "docx" | "pptx" | "xlsx" => office_text(path, ext, limits.max_chars).map(|t| (Method::OfficeXml, t)),
        _ if PLAIN_TEXT.contains(&ext) => plain_text(path, limits.max_chars).map(|t| (Method::PlainText, t)),
        _ => None,
    };
    let name = name_words(&item.name);
    match inside.map(|(m, t)| (m, truncate(&collapse_whitespace(&t), limits.max_chars))) {
        Some((method, text)) if text.chars().filter(|c| c.is_alphabetic()).count() >= 20 => {
            Content { method, text: format!("{name}. {text}") }
        }
        _ => Content { method: Method::NameOnly, text: name },
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

/// The PDF's text layer. pdf-extract can panic or take very long on odd
/// files, so it runs on its own thread and is abandoned after `timeout`.
fn pdf_text(path: &Path, timeout: Duration) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let text = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes).ok()).ok().flatten();
        let _ = tx.send(text);
    });
    rx.recv_timeout(timeout).ok().flatten()
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
