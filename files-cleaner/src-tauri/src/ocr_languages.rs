//! OCR's languages: Tesseract's language files, pinned by revision, size
//! and SHA-256, downloaded into a folder in the app data folder each time
//! the app opens without them. They're a few MB, so unlike the Grouping
//! Models they come without asking. Until they're in, scanned PDFs are read
//! by name. A file is moved into place only once its SHA-256 matches, so
//! the languages are installed exactly when every file exists.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// One language file from tessdata_fast, Tesseract's fast LSTM models.
struct Language {
    file_name: &'static str,
    url: &'static str,
    size: u64,
    sha256: &'static str,
}

/// Tesseract reads every one of them at once, English tried first. Each
/// added language makes OCR slower.
const LANGUAGES: [Language; 2] = [
    Language {
        file_name: "eng.traineddata",
        url: "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/87416418657359cb625c412a48b6e1d6d41c29bd/eng.traineddata",
        size: 4_113_088,
        sha256: "7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2",
    },
    Language {
        file_name: "vie.traineddata",
        url: "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/87416418657359cb625c412a48b6e1d6d41c29bd/vie.traineddata",
        size: 531_275,
        sha256: "79df64caf7bcfb2a27df5042ecb6121e196eada34da774956995747636d5bfa1",
    },
];

/// A connection that sends nothing for this long fails; the next open tries again.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// The folder of OCR's language files, once every one is installed.
pub fn installed(folder: &Path) -> Option<PathBuf> {
    LANGUAGES
        .iter()
        .all(|language| folder.join(language.file_name).is_file())
        .then(|| folder.to_path_buf())
}

/// Downloads the missing language files into `folder` on their own thread.
/// A failure only leaves OCR off until the app opens again.
pub fn install(folder: PathBuf) {
    if installed(&folder).is_some() {
        return;
    }
    std::thread::spawn(move || {
        if fs::create_dir_all(&folder).is_err() {
            return;
        }
        for language in &LANGUAGES {
            if !folder.join(language.file_name).is_file() {
                let _ = fetch(language, &folder);
            }
        }
    });
}

/// Downloads `language` whole, then moves it into place if it's the pinned file.
fn fetch(language: &Language, folder: &Path) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(READ_TIMEOUT))
        .timeout_per_read(Some(READ_TIMEOUT))
        .build()
        .into();
    let mut bytes = Vec::with_capacity(language.size as usize);
    agent
        .get(language.url)
        .call()
        .map_err(|e| e.to_string())?
        .into_body()
        .into_reader()
        .take(language.size + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let sha256: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if bytes.len() as u64 != language.size || sha256 != language.sha256 {
        return Err(format!("{} downloaded damaged", language.file_name));
    }
    let partial = folder.join(format!("{}.part", language.file_name));
    fs::write(&partial, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&partial, folder.join(language.file_name)).map_err(|e| e.to_string())
}
