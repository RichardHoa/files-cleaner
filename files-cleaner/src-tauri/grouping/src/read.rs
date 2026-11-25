//! Files are only ever opened for reading.

use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{ocr, trace};

#[derive(Debug, Clone)]
pub struct Content {
    pub name: String,
    pub file_name: String,
    pub body: String,
    /// No text layer, and OCR couldn't read it either.
    pub scanned: bool,
}

/// Raise it when a file would be read differently, so cards from the old
/// reading aren't used.
pub const READER_VERSION: u32 = 4;

const MAX_CHARS: usize = 2000;
const PDF_TIMEOUT: Duration = Duration::from_secs(10);
/// The rest of the document is never decoded.
const PDF_PAGES: i32 = 3;
const OCR_TIMEOUT: Duration = Duration::from_secs(45);
/// OCR takes a second or more per page.
const OCR_PAGES: i32 = 2;
/// Pixels per point (216 dpi), capped by `OCR_MAX_PIXELS` on a side.
const OCR_SCALE: f32 = 3.0;
const OCR_MAX_PIXELS: i32 = 3000;

const PLAIN_TEXT: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "xml", "log", "yaml", "yml", "toml", "ini",
    "tex", "rst", "org", "srt", "vtt", "rs", "py", "js", "mjs", "ts", "jsx", "tsx", "java", "kt",
    "swift", "c", "h", "cc", "cpp", "hpp", "cs", "go", "rb", "php", "sh", "zsh", "bash", "ps1",
    "sql", "css", "scss", "r", "m", "lua", "dart", "scala", "svelte", "vue", "ipynb",
];

/// Below this share the text layer is garbled, as when a font maps glyphs to
/// the wrong characters ("Collocations" reads `.;!!;)7°—;J©`).
const PDF_PLAIN_WORDS: f32 = 0.5;

/// So a zip bomb can't exhaust memory.
const MAX_XML_BYTES: u64 = 4 * 1024 * 1024;

/// Larger than for other text: it can come after long style and script blocks.
const MAX_HTML_BYTES: u64 = 1024 * 1024;

const HTML_HIDDEN: &[&str] = &[
    "script", "style", "template", "svg", "math", "noscript", "iframe", "object", "canvas",
];

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

pub fn read(path: &Path, name: &str, pdfium: Option<&Path>, tessdata: Option<&Path>) -> Content {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    let pdf = ext == "pdf";
    let inside = match ext.as_str() {
        "pdf" => pdfium.and_then(|pdfium| pdf_text(pdfium, path, PdfReading::TextLayer, None)),
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
    // Next to no text means pages of images; garbled text still shows right.
    // Both can be read as pictures.
    match pdfium
        .zip(tessdata)
        .and_then(|(pdfium, tessdata)| pdf_text(pdfium, path, PdfReading::Ocr, Some(tessdata)))
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

/// Dates and amounts count as plain, so `.,:/-` is allowed between digits.
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

const PDF_POLL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy)]
enum PdfReading {
    TextLayer,
    Ocr,
}

impl PdfReading {
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

/// PDFium and Tesseract are C++ and can crash or hang on odd files, so a child
/// process runs them and can be killed.
fn pdf_text(
    pdfium: &Path,
    path: &Path,
    reading: PdfReading,
    tessdata: Option<&Path>,
) -> Option<String> {
    let mut child = Command::new(std::env::current_exe().ok()?)
        .arg(reading.flag())
        .arg(pdfium)
        .arg(path)
        .args(tessdata)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // Piped only when tracing reads it.
        .stderr(if trace::enabled() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .ok()?;
    let started = Instant::now();
    // The output is capped well under a pipe's buffer, so the child never
    // blocks while we wait.
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

/// `main` must call this first: the app is its own PDF child.
pub fn pdf_child_main() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let reading = [PdfReading::TextLayer, PdfReading::Ocr]
        .into_iter()
        .find(|r| args.get(1).is_some_and(|a| a == r.flag()))?;
    let (Some(pdfium), Some(path)) = (args.get(2), args.get(3)) else {
        return Some(1);
    };
    let tessdata = args.get(4).map(Path::new);
    // A crash here only ends this process; the parent sees a failure.
    Some(
        match pdf_child(Path::new(pdfium), Path::new(path), reading, tessdata) {
            Ok(()) => 0,
            Err(_) => 1,
        },
    )
}

/// As each step ends, so the parent sees how far a child it killed got.
fn pdf_child(
    pdfium: &Path,
    path: &Path,
    reading: PdfReading,
    tessdata: Option<&Path>,
) -> Result<(), String> {
    let step = Instant::now();
    let pdfium = Pdfium::new(Pdfium::bind_to_library(pdfium).map_err(|e| e.to_string())?);
    // Seekable, not read into memory: PDFium reads only the pages asked for, so
    // a 2 GB PDF costs about what a 2 MB one does.
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
    let (count, ocr) = match reading {
        PdfReading::TextLayer => (PDF_PAGES, None),
        PdfReading::Ocr => {
            let step = Instant::now();
            let ocr = tessdata
                .and_then(ocr::Reader::new)
                .ok_or("no OCR languages")?;
            eprintln!("load Tesseract: {}", trace::secs(step.elapsed()));
            (OCR_PAGES, Some(ocr))
        }
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
                let ocr = ocr.as_ref().ok_or("no OCR")?;
                text.push_str(&ocr.recognize(&image).ok_or("OCR failed")?);
                eprintln!("page {}: OCR {}", index + 1, trace::secs(step.elapsed()));
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

/// Word splits words across runs, so runs inside one paragraph join directly.
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

/// The models must never see styles, scripts or markup.
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
