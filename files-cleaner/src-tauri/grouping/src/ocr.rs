//! Reading the text in a picture of a page, for PDFs with no text layer
//! (scans) or a garbled one, with the engine each OS has: Vision on macOS,
//! Windows.Media.Ocr on Windows, and elsewhere the `tesseract` command when
//! it's installed. Runs in the PDF child process (`read::pdf_child_main`).

/// One rendered page, RGBA, 8 bits a channel, rows top to bottom.
pub struct Page {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Page {
    #[cfg(not(windows))]
    fn png(&self) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&self.rgba).ok()?;
        writer.finish().ok()?;
        Some(out)
    }
}

/// The page's text, one line per line found; `None` when there's no OCR
/// engine or it failed.
#[cfg(target_os = "macos")]
pub fn recognize(page: &Page) -> Option<String> {
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };

    // Timings go to stderr for the parent's trace (`read::pdf_child`).
    let step = std::time::Instant::now();
    let data = NSData::with_bytes(&page.png()?);
    eprintln!("  PNG encode {}", crate::trace::secs(step.elapsed()));
    let step = std::time::Instant::now();
    let handler = VNImageRequestHandler::initWithData_options(
        VNImageRequestHandler::alloc(),
        &data,
        &NSDictionary::new(),
    );
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    request.setUsesLanguageCorrection(true);
    // Before macOS 13 Vision reads English only.
    if objc2::available!(macos = 13.0) {
        request.setAutomaticallyDetectsLanguage(true);
    }
    let as_request: &VNRequest = &request;
    handler
        .performRequests_error(&NSArray::from_slice(&[as_request]))
        .ok()?;
    let lines: Vec<String> = request
        .results()?
        .iter()
        .filter_map(|line| line.topCandidates(1).firstObject())
        .map(|text| text.string().to_string())
        .collect();
    eprintln!(
        "  Vision {} ({} lines)",
        crate::trace::secs(step.elapsed()),
        lines.len()
    );
    Some(lines.join("\n"))
}

/// The page's text, one line per line found; `None` when there's no OCR
/// engine or it failed. Reads the first of the user's languages that has an
/// OCR pack installed.
#[cfg(windows)]
pub fn recognize(page: &Page) -> Option<String> {
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    let max = OcrEngine::MaxImageDimension().ok()?;
    if page.width > max || page.height > max {
        return None;
    }
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;
    let bgra: Vec<u8> = page
        .rgba
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0], p[3]])
        .collect();
    let writer = DataWriter::new().ok()?;
    writer.WriteBytes(&bgra).ok()?;
    let buffer = writer.DetachBuffer().ok()?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        page.width as i32,
        page.height as i32,
        BitmapAlphaMode::Premultiplied,
    )
    .ok()?;
    let result = engine.RecognizeAsync(&bitmap).ok()?.join().ok()?;
    let lines: Vec<String> = result
        .Lines()
        .ok()?
        .into_iter()
        .filter_map(|line| line.Text().ok())
        .map(|t| t.to_string())
        .collect();
    Some(lines.join("\n"))
}

/// The page's text from the `tesseract` command, in every language it has
/// installed; `None` when it isn't installed or failed.
#[cfg(not(any(target_os = "macos", windows)))]
pub fn recognize(page: &Page) -> Option<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::sync::OnceLock;

    static LANGUAGES: OnceLock<Option<String>> = OnceLock::new();
    let languages = LANGUAGES.get_or_init(tesseract_languages).as_deref()?;
    let png = page.png()?;
    let mut child = Command::new("tesseract")
        .args(["stdin", "stdout", "-l", languages])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Tesseract reads the whole image before it writes anything, so the
    // pipes can't both fill up.
    child.stdin.take()?.write_all(&png).ok()?;
    let out = child.wait_with_output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The installed Tesseract languages joined for `-l`, as `eng+vie`, leaving
/// out the ones that aren't scripts of text.
#[cfg(not(any(target_os = "macos", windows)))]
fn tesseract_languages() -> Option<String> {
    let out = std::process::Command::new("tesseract")
        .arg("--list-langs")
        .output()
        .ok()?;
    // Older versions list them on stderr; the first line is a header.
    let listed = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let languages: Vec<&str> = listed
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains(' ') && !matches!(*l, "osd" | "equ"))
        .collect();
    (!languages.is_empty()).then(|| languages.join("+"))
}
