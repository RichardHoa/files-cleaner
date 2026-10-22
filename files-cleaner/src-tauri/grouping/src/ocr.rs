//! Reading the text in a picture of a page, for PDFs with no text layer
//! (scans) or a garbled one, with Tesseract, built into the app on every
//! OS. Its language files are downloaded by the app into one folder
//! (`OcrLanguages` in the app crate). Runs in the PDF child process
//! (`read::pdf_child_main`).

use std::path::Path;

use tesseract_rs::TesseractAPI;

/// One rendered page, RGBA, 8 bits a channel, rows top to bottom.
pub struct Page {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Tesseract, loaded with every language in the folder it was made from.
pub struct Reader(TesseractAPI);

impl Reader {
    /// Tesseract with every `.traineddata` file in `tessdata`, English
    /// first so it's tried first; `None` when there are none or they don't
    /// load.
    pub fn new(tessdata: &Path) -> Option<Reader> {
        let mut languages: Vec<String> = std::fs::read_dir(tessdata)
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                name.strip_suffix(".traineddata").map(str::to_string)
            })
            .collect();
        languages.sort_by_key(|l| (l != "eng", l.clone()));
        if languages.is_empty() {
            return None;
        }
        let api = TesseractAPI::new();
        api.init(tessdata, &languages.join("+")).ok()?;
        Some(Reader(api))
    }

    /// The page's text, one line per line found; `None` when Tesseract failed.
    pub fn recognize(&self, page: &Page) -> Option<String> {
        let (width, height) = (page.width as i32, page.height as i32);
        self.0
            .set_image(&page.rgba, width, height, 4, width * 4)
            .ok()?;
        self.0.get_utf8_text().ok()
    }
}
