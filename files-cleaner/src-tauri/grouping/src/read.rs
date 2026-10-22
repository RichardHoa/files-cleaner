//! Reading a short text for each file: its name, plus what's inside it when
//! the format is one we can read. PDFs without a usable text layer are read
//! by OCR (`ocr.rs`). Files are only ever opened for reading.

use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{ocr, trace};

/// What the models are given about one file.
#[derive(Debug, Clone)]
pub struct Content {
    /// The file name as words.
    pub name: String,
    /// The file name as it is on disk, which names its trace.
    pub file_name: String,
    /// What's inside, at most `MAX_CHARS`; empty when read by name only.
    pub body: String,
    /// A PDF with no text layer, such as a scan, that OCR couldn't read
    /// either: there's no OCR engine, or its pages are pictures.
    pub scanned: bool,
}

/// Changes when the same file would be read differently, so cards made
/// from the old reading aren't used.
pub const READER_VERSION: u32 = 3;

/// Most characters of content read from each file.
const MAX_CHARS: usize = 2000;
/// A PDF whose text isn't out by then is read by its name.
const PDF_TIMEOUT: Duration = Duration::from_secs(10);
/// Pages of a PDF read for its text; the rest of the document is never decoded.
const PDF_PAGES: i32 = 3;
/// A PDF whose pages aren't through OCR by then is read without them.
const OCR_TIMEOUT: Duration = Duration::from_secs(45);
/// Pages of a PDF read by OCR, which takes a second or more each.
const OCR_PAGES: i32 = 2;
/// Pages are rendered for OCR at this many pixels per point (216 dpi),
/// and at most `OCR_MAX_PIXELS` on a side.
const OCR_SCALE: f32 = 3.0;
const OCR_MAX_PIXELS: i32 = 3000;

const PLAIN_TEXT: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "xml", "log", "yaml", "yml", "toml", "ini",
    "tex", "rst", "org", "srt", "vtt", "rs", "py", "js", "mjs", "ts", "jsx", "tsx", "java", "kt",
    "swift", "c", "h", "cc", "cpp", "hpp", "cs", "go", "rb", "php", "sh", "zsh", "bash", "ps1",
    "sql", "css", "scss", "r", "m", "lua", "dart", "scala", "svelte", "vue", "ipynb",
];

/// Of the words in a PDF's text, the fewest that must be plain words or
/// numbers. Below it the text layer is garbled, as when a font maps its
/// glyphs to the wrong characters ("Collocations" comes out as
/// `.;!!;)7°—;J©`), and the PDF is read by its name.
const PDF_PLAIN_WORDS: f32 = 0.5;

/// Most bytes read from one part of an Office file, so a zip bomb can't
/// exhaust memory.
const MAX_XML_BYTES: u64 = 4 * 1024 * 1024;

/// Most bytes read from an HTML file: its text can come after long style
/// and script blocks.
const MAX_HTML_BYTES: u64 = 1024 * 1024;

/// HTML elements whose content is never shown as text.
const HTML_HIDDEN: &[&str] = &[
    "script", "style", "template", "svg", "math", "noscript", "iframe", "object", "canvas",
];

/// HTML elements that start a new line of text.
const HTML_BLOCKS: &[&str] = &[
    "title",
    "p",
    "br",
    "div",
    "section",
    "article",
    "header",
    "footer",
    "main",
    "nav",
    "aside",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "tr",
    "td",
    "th",
    "dt",
    "dd",
    "pre",
    "blockquote",
    "table",
    "ul",
    "ol",
    "hr",
    "figcaption",
];

/// Reads the file at `path` named `name`. `pdfium` is the PDFium library;
/// without it, PDFs are read by name.
pub fn read(path: &Path, name: &str, pdfium: Option<&Path>) -> Content {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    let pdf = ext == "pdf";
    let inside = match ext.as_str() {
        "pdf" => pdfium.and_then(|pdfium| pdf_text(pdfium, path, PdfReading::TextLayer)),
        "docx" | "pptx" | "xlsx" => office_text(path, &ext),
        "html" | "htm" => html_text(path),
        _ if PLAIN_TEXT.contains(&ext.as_str()) => plain_text(path),
        _ => None,
    };
    let (file_name, name) = (name.to_string(), name_words(name));
    let clean = |text: String| truncate(&collapse_whitespace(&text), MAX_CHARS);
    let readable = |body: &str| body.chars().filter(|c| c.is_alphabetic()).count() >= 20;
    let Some(body) = inside.map(clean) else {
        return Content {
            name,
            file_name,
            body: String::new(),
            scanned: false,
        };
    };
    let has_text = readable(&body);
    if has_text && (!pdf || plain_word_share(&body) >= PDF_PLAIN_WORDS) {
        return Content {
            name,
            file_name,
            body,
            scanned: false,
        };
    }
    if !pdf {
        return Content {
            name,
            file_name,
            body: String::new(),
            scanned: false,
        };
    }
    // A PDF that opened with next to no text is pages of images, as a scan
    // is; one with garbled text still shows it right. Either way the pages
    // are read as pictures.
    match pdfium
        .and_then(|pdfium| pdf_text(pdfium, path, PdfReading::Ocr))
        .map(clean)
    {
        Some(body) if readable(&body) => Content {
            name,
            file_name,
            body,
            scanned: false,
        },
        _ => Content {
            name,
            file_name,
            body: String::new(),
            scanned: !has_text,
        },
    }
}

/// The share of `text`'s words that, without the punctuation around them,
/// are all letters or all digits (with `.,:/-` between them, as in dates).
fn plain_word_share(text: &str) -> f32 {
    let words: Vec<&str> = text.split_whitespace().collect();
    let plain = words
        .iter()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| {
            !w.is_empty()
                && (w.chars().all(char::is_alphabetic)
                    || w.chars().all(|c| c.is_ascii_digit() || ".,:/-".contains(c)))
        })
        .count();
    plain as f32 / words.len().max(1) as f32
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
                let digit_edge =
                    p.is_alphabetic() != c.is_alphabetic() && (p.is_numeric() || c.is_numeric());
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

/// How a PDF is read in the child process: see `pdf_child_main`.
#[derive(Clone, Copy)]
enum PdfReading {
    /// Its text layer, from `PDF_PAGES` pages.
    TextLayer,
    /// Its pages rendered and read by OCR, `OCR_PAGES` of them.
    Ocr,
}

impl PdfReading {
    /// The flag the app is started with to read one PDF this way.
    fn flag(self) -> &'static str {
        match self {
            PdfReading::TextLayer => "--read-pdf",
            PdfReading::Ocr => "--ocr-pdf",
        }
    }

    fn timeout(self) -> Duration {
        match self {
            PdfReading::TextLayer => PDF_TIMEOUT,
            PdfReading::Ocr => OCR_TIMEOUT,
        }
    }
}

/// The PDF's text, read the given way. PDFium is C++ and can crash or take
/// long on odd files, so it runs in a child process of the app itself
/// (`PdfReading::flag`), which is killed after `PdfReading::timeout`; the
/// OS then frees everything it held.
fn pdf_text(pdfium: &Path, path: &Path, reading: PdfReading) -> Option<String> {
    let mut child = Command::new(std::env::current_exe().ok()?)
        .arg(reading.flag())
        .arg(pdfium)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // The child's timings, on stderr, are kept only when tracing.
        .stderr(if trace::enabled() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .ok()?;
    let started = Instant::now();
    // The child prints at most `MAX_CHARS` characters, and a few lines of
    // timings, well under a pipe's buffer, so it never blocks on a full
    // pipe while we wait.
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break "done",
            Ok(Some(_)) | Err(_) => break "failed",
            Ok(None) if started.elapsed() >= reading.timeout() => {
                let _ = child.kill();
                let _ = child.wait();
                break "timed out";
            }
            Ok(None) => std::thread::sleep(PDF_POLL),
        }
    };
    if let Some(mut stderr) = child.stderr.take() {
        let mut steps = String::new();
        let _ = stderr.read_to_string(&mut steps);
        let steps: String = steps.lines().map(|l| format!("    {l}\n")).collect();
        trace::time(&format!(
            "  {}: {outcome} after {}\n{steps}",
            reading.flag(),
            trace::secs(started.elapsed())
        ));
    }
    if outcome != "done" {
        return None;
    }
    let mut text = String::new();
    child.stdout.take()?.read_to_string(&mut text).ok()?;
    Some(text)
}

/// When the app was started to read one PDF, reads it and says how the
/// process should exit; otherwise `None`, and the app starts as usual. The
/// app's `main` calls this first. The text of the first pages, at most
/// `MAX_CHARS` characters, goes to stdout.
pub fn pdf_child_main() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let reading = [PdfReading::TextLayer, PdfReading::Ocr]
        .into_iter()
        .find(|r| args.get(1).is_some_and(|a| a == r.flag()))?;
    let (Some(pdfium), Some(path)) = (args.get(2), args.get(3)) else {
        return Some(1);
    };
    // A panic or a crash in PDFium just ends this process; the parent sees a failure.
    Some(
        match pdf_child(Path::new(pdfium), Path::new(path), reading) {
            Ok(()) => 0,
            Err(_) => 1,
        },
    )
}

/// Each step's time goes to stderr as it ends, so the parent sees how far
/// a child it killed got.
fn pdf_child(pdfium: &Path, path: &Path, reading: PdfReading) -> Result<(), String> {
    let step = Instant::now();
    let pdfium = Pdfium::new(Pdfium::bind_to_library(pdfium).map_err(|e| e.to_string())?);
    // Opened through a seekable reader, not read into memory: PDFium reads
    // the cross-reference table and then only the objects of the pages it
    // is asked for, so a 2 GB PDF costs about what a 2 MB one does.
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| e.to_string())?;
    let pages = doc.pages();
    eprintln!(
        "load PDFium and open ({} pages): {}",
        pages.len(),
        trace::secs(step.elapsed())
    );
    let render = PdfRenderConfig::new()
        .scale_page_by_factor(OCR_SCALE)
        .set_maximum_width(OCR_MAX_PIXELS)
        .set_maximum_height(OCR_MAX_PIXELS);
    let count = match reading {
        PdfReading::TextLayer => PDF_PAGES,
        PdfReading::Ocr => OCR_PAGES,
    };
    let mut text = String::new();
    for index in 0..pages.len().min(count) {
        let step = Instant::now();
        let page = pages.get(index).map_err(|e| e.to_string())?;
        match reading {
            PdfReading::TextLayer => {
                text.push_str(&page.text().map_err(|e| e.to_string())?.all());
                eprintln!(
                    "page {}: text layer {}",
                    index + 1,
                    trace::secs(step.elapsed())
                );
            }
            PdfReading::Ocr => {
                let bitmap = page
                    .render_with_config(&render)
                    .map_err(|e| e.to_string())?;
                eprintln!(
                    "page {}: render {}x{} {}",
                    index + 1,
                    bitmap.width(),
                    bitmap.height(),
                    trace::secs(step.elapsed())
                );
                let step = Instant::now();
                let image = ocr::Page {
                    width: bitmap.width() as u32,
                    height: bitmap.height() as u32,
                    rgba: bitmap.as_rgba_bytes(),
                };
                eprintln!(
                    "page {}: to RGBA {}",
                    index + 1,
                    trace::secs(step.elapsed())
                );
                // No OCR engine reads one page if it reads none.
                text.push_str(&ocr::recognize(&image).ok_or("OCR failed")?);
            }
        }
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
    std::fs::File::open(path)
        .ok()?
        .take(MAX_CHARS as u64 * 4)
        .read_to_end(&mut bytes)
        .ok()?;
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
        n.trim_start_matches(|c: char| !c.is_ascii_digit())
            .trim_end_matches(".xml")
            .parse::<u32>()
            .unwrap_or(0)
    });
    let mut text = String::new();
    for part in parts {
        let Ok(entry) = archive.by_name(&part) else {
            continue;
        };
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
        let Some(gt) = rest[lt..].find('>') else {
            return;
        };
        let tag = &rest[lt + 1..lt + gt];
        let local = tag
            .trim_start_matches('/')
            .split([' ', '/'])
            .next()
            .unwrap_or("");
        let local = local.rsplit(':').next().unwrap_or(local);
        if matches!(local, "p" | "br" | "tab" | "si" | "c" | "tc" | "cr") && !out.ends_with(' ') {
            out.push(' ');
            chars += 1;
        }
        rest = &rest[lt + gt + 1..];
    }
}

fn html_text(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_HTML_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(strip_html(&String::from_utf8_lossy(&bytes)))
}

/// The text of `html` as a browser shows it, at most about `MAX_CHARS`
/// characters: tags, comments and the content of `HTML_HIDDEN` elements
/// are dropped, so the models never see styles, scripts or markup.
fn strip_html(html: &str) -> String {
    // ASCII lowercasing keeps every byte where it was, so offsets found in
    // `lower` index `html` too.
    let lower = html.to_ascii_lowercase();
    let mut out = String::new();
    let mut chars = 0;
    let mut at = 0;
    while let Some(lt) = lower[at..].find('<').map(|i| at + i) {
        let text = decode_entities(&html[at..lt]);
        chars += text.chars().count();
        out.push_str(&text);
        if chars >= MAX_CHARS {
            return out;
        }
        if lower[lt..].starts_with("<!--") {
            at = lower[lt..].find("-->").map_or(html.len(), |i| lt + i + 3);
            continue;
        }
        let Some(gt) = lower[lt..].find('>').map(|i| lt + i) else {
            return out;
        };
        let tag = &lower[lt + 1..gt];
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_ascii_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        at = gt + 1;
        if !tag.starts_with('/') && HTML_HIDDEN.contains(&name) {
            let close = format!("</{name}");
            at = lower[at..]
                .find(&close)
                .and_then(|i| lower[at + i..].find('>').map(|g| at + i + g + 1))
                .unwrap_or(html.len());
        } else if HTML_BLOCKS.contains(&name) && !out.ends_with('\n') {
            out.push('\n');
            chars += 1;
        }
    }
    out.push_str(&decode_entities(&html[at..]));
    out
}

/// Decodes the named entities of XML, `&nbsp;`, and numeric references.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').filter(|&semi| semi <= 10).and_then(|semi| {
            let c = match &rest[1..semi] {
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "amp" => '&',
                "nbsp" => ' ',
                name => {
                    let code = match name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => name.strip_prefix('#')?.parse().ok()?,
                    };
                    char::from_u32(code)?
                }
            };
            Some((c, semi + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_is_read_as_its_text_only() {
        let html = "<!doctype html><html><head><meta charset=utf8><style>body{color:#141413}</style>\
            <title>Bias Test</title><script>let a = 1 < 2;</script></head><body><!-- note -->\
            <h1>Shuffle&nbsp;Kinds</h1><p>Caf&#233; &amp; <b>bias</b> results</p><svg><text>x</text></svg></body></html>";
        assert_eq!(
            collapse_whitespace(&strip_html(html)),
            "Bias Test\nShuffle Kinds\nCafé & bias results"
        );
    }
}
