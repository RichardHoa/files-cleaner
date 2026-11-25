//! For PDFs with no text layer or a garbled one. Runs in the PDF child process
//! (`read::pdf_child_main`), as Tesseract can crash.

use std::path::Path;

use tesseract_rs::TesseractAPI;

pub struct Page {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct Reader(TesseractAPI);

impl Reader {
    /// English first, so it is tried first.
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

    pub fn recognize(&self, page: &Page) -> Option<String> {
        let (width, height) = (page.width as i32, page.height as i32);
        self.0
            .set_image(&page.rgba, width, height, 4, width * 4)
            .ok()?;
        self.0.get_utf8_text().ok()
    }
}
