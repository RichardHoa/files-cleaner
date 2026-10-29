//! The Grouping Models (ADR-0004): two official GGUF files, pinned by
//! revision, size and SHA-256, downloaded on the user's say-so into a
//! folder in the app data folder. The download runs on its own thread, so
//! the user can scan and clean meanwhile.
//!
//! Each model downloads into `<file>.part`, resumed with an HTTP Range
//! request after a pause or an interruption, and is moved to `<file>` only
//! once its SHA-256 matches. So a model is installed exactly when `<file>`
//! exists, and the state can always be read from the folder.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

/// The event the download's status is sent on, to every window.
pub const STATUS_EVENT: &str = "grouping-models";

/// How often, at most, the status is sent while downloading.
const STATUS_EVERY: Duration = Duration::from_millis(250);
/// A connection that sends nothing for this long fails, so the user can resume.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// One model file, pinned at a fixed repository revision, never a branch.
struct Model {
    /// What it does, for error messages: "the file-reading model".
    role: &'static str,
    /// The model's own name, shown in Settings.
    name: &'static str,
    file_name: &'static str,
    url: &'static str,
    size: u64,
    sha256: &'static str,
}

const MODELS: [Model; 2] = [
    Model {
        role: "file-reading model",
        name: "Qwen2.5-1.5B-Instruct",
        file_name: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/91cad51170dc346986eccefdc2dd33a9da36ead9/qwen2.5-1.5b-instruct-q4_k_m.gguf",
        size: 1_117_320_736,
        sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
    },
    Model {
        role: "comparison model",
        name: "Qwen3-Embedding-0.6B",
        file_name: "Qwen3-Embedding-0.6B-Q8_0.gguf",
        url: "https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/370f27d7550e0def9b39c1f16d3fbaa13aa67728/Qwen3-Embedding-0.6B-Q8_0.gguf",
        size: 639_150_592,
        sha256: "06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439",
    },
];

/// What a running download has been asked to do, checked between reads.
const RUN: u8 = 0;
const PAUSE: u8 = 1;
const CANCEL: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    NotInstalled,
    Downloading,
    Paused,
    Installed,
}

/// The Grouping Models as the UI shows them.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: State,
    /// Bytes on disk so far, out of `total`.
    pub downloaded: u64,
    pub total: u64,
    /// Why the last download stopped, when it failed rather than finished.
    pub error: Option<String>,
    /// Each model, the file-reading one first.
    pub models: Vec<ModelStatus>,
}

/// One model as Settings shows it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub name: &'static str,
    /// Bytes on disk, installed or partly downloaded.
    pub size: u64,
    /// Its file, or its partial file while downloading; `None` when neither is on disk.
    pub path: Option<PathBuf>,
}

/// Kept by the app: the models folder and the running download, if any.
pub struct GroupingModels {
    folder: PathBuf,
    app: AppHandle,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// The running download's instruction (RUN, PAUSE or CANCEL), until its thread ends.
    running: Option<Arc<AtomicU8>>,
    error: Option<String>,
}

/// Why a download stopped before finishing.
enum Stopped {
    /// Pause or cancel was asked for.
    Asked,
    Failed(String),
}

impl GroupingModels {
    pub fn new(app: AppHandle, folder: PathBuf) -> Self {
        GroupingModels {
            folder,
            app,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// True once both models are downloaded and checked.
    pub fn installed(&self) -> bool {
        MODELS.iter().all(|model| self.file(model).is_file())
    }

    /// The two model files, the card writer's then the embedding model's,
    /// once both are installed.
    pub fn files(&self) -> Option<(PathBuf, PathBuf)> {
        self.installed()
            .then(|| (self.file(&MODELS[0]), self.file(&MODELS[1])))
    }

    pub fn status(&self) -> Status {
        let inner = self.inner.lock().unwrap();
        self.status_of(&inner)
    }

    fn status_of(&self, inner: &Inner) -> Status {
        let total = MODELS.iter().map(|model| model.size).sum();
        let downloaded = MODELS.iter().map(|model| self.on_disk(model)).sum();
        let state = match inner
            .running
            .as_ref()
            .map(|asked| asked.load(Ordering::SeqCst))
        {
            Some(RUN) => State::Downloading,
            Some(PAUSE) => State::Paused,
            // Its files go once its thread sees the cancel.
            Some(_) => {
                return Status {
                    state: State::NotInstalled,
                    downloaded: 0,
                    total,
                    error: None,
                    models: Vec::new(),
                }
            }
            None if self.installed() => State::Installed,
            None if downloaded > 0 => State::Paused,
            None => State::NotInstalled,
        };
        Status {
            state,
            downloaded,
            total,
            error: inner.error.clone(),
            models: MODELS
                .iter()
                .map(|model| self.model_status(model))
                .collect(),
        }
    }

    fn model_status(&self, model: &Model) -> ModelStatus {
        let path = [self.file(model), self.partial(model)]
            .into_iter()
            .find(|path| path.is_file());
        ModelStatus {
            name: model.name,
            size: self.on_disk(model),
            path,
        }
    }

    /// Starts the download, or resumes it from where it stopped, after
    /// checking there's room for what's left plus about 10%.
    pub fn download(self: &Arc<Self>) -> Result<Status, String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(asked) = &inner.running {
            // Paused or cancelled, but its thread hasn't noticed yet: it carries on.
            asked.store(RUN, Ordering::SeqCst);
        } else if !self.installed() {
            fs::create_dir_all(&self.folder).map_err(|e| e.to_string())?;
            let left: u64 = MODELS
                .iter()
                .map(|model| model.size - self.on_disk(model))
                .sum();
            let needed = left + left / 10;
            let free = fs4::available_space(&self.folder).map_err(|e| e.to_string())?;
            if free < needed {
                return Err(format!(
                    "Not enough free space for the grouping models: they need {} and this disk has {} free.",
                    gigabytes(needed),
                    gigabytes(free)
                ));
            }
            let asked = Arc::new(AtomicU8::new(RUN));
            inner.running = Some(Arc::clone(&asked));
            inner.error = None;
            let models = Arc::clone(self);
            std::thread::spawn(move || models.work(&asked));
        }
        let status = self.status_of(&inner);
        drop(inner);
        self.send(&status);
        Ok(status)
    }

    /// Stops the download, keeping what's downloaded so far.
    pub fn pause(&self) -> Status {
        self.ask(PAUSE)
    }

    /// Cancels the download, or removes the installed models: either way,
    /// every model file goes.
    pub fn remove(&self) -> Result<Status, String> {
        // All under the lock, so a download can't start, or the running one end, in between.
        let mut inner = self.inner.lock().unwrap();
        inner.error = None;
        match &inner.running {
            Some(asked) => asked.store(CANCEL, Ordering::SeqCst),
            None => self.delete_files().map_err(|e| e.to_string())?,
        }
        let status = self.status_of(&inner);
        drop(inner);
        self.send(&status);
        Ok(status)
    }

    fn ask(&self, what: u8) -> Status {
        let inner = self.inner.lock().unwrap();
        if let Some(asked) = &inner.running {
            asked.store(what, Ordering::SeqCst);
        }
        let status = self.status_of(&inner);
        drop(inner);
        self.send(&status);
        status
    }

    /// The download thread: fetches each model in turn until all are in,
    /// or it's paused, cancelled, or fails.
    fn work(&self, asked: &AtomicU8) {
        let mut last_sent = Instant::now();
        let mut progress = || {
            if last_sent.elapsed() >= STATUS_EVERY {
                last_sent = Instant::now();
                self.send(&self.status());
            }
        };
        let status = loop {
            let result = MODELS
                .iter()
                .try_for_each(|model| self.fetch(model, asked, &mut progress));
            // Under the lock, so a resume can't slip in between deciding to stop and stopping.
            let mut inner = self.inner.lock().unwrap();
            match (asked.load(Ordering::SeqCst), result) {
                (RUN, Err(Stopped::Asked)) => continue,
                // However it ended, a cancel removes everything.
                (CANCEL, _) => {
                    if let Err(e) = self.delete_files() {
                        inner.error = Some(format!("Couldn't delete the downloaded files: {e}"));
                    }
                }
                (_, Err(Stopped::Failed(message))) => inner.error = Some(message),
                _ => {}
            }
            inner.running = None;
            break self.status_of(&inner);
        };
        self.send(&status);
    }

    /// Downloads `model` into its partial file, from where that stopped,
    /// then moves it into place if its SHA-256 matches.
    fn fetch(
        &self,
        model: &Model,
        asked: &AtomicU8,
        progress: &mut impl FnMut(),
    ) -> Result<(), Stopped> {
        let failed = |e: &dyn std::fmt::Display| {
            Stopped::Failed(format!("Downloading the {} failed: {e}", model.role))
        };
        let file = self.file(model);
        if file.is_file() {
            return Ok(());
        }
        let partial = self.partial(model);
        let mut offset = fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
        if offset > model.size {
            fs::remove_file(&partial).map_err(|e| failed(&e))?;
            offset = 0;
        }
        if offset < model.size {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_connect(Some(READ_TIMEOUT))
                .timeout_per_read(Some(READ_TIMEOUT))
                .http_status_as_error(false)
                .build()
                .into();
            let response = agent
                .get(model.url)
                .header("Range", format!("bytes={offset}-"))
                .call()
                .map_err(|e| failed(&e))?;
            let resumed = match response.status().as_u16() {
                206 => {
                    let range = response
                        .headers()
                        .get("content-range")
                        .and_then(|v| v.to_str().ok());
                    if !range.is_some_and(|range| range.starts_with(&format!("bytes {offset}-"))) {
                        return Err(failed(&"the server sent the wrong part of the file"));
                    }
                    true
                }
                // The server ignored the range: start again.
                200 => false,
                code => return Err(failed(&format!("the server answered {code}"))),
            };
            let mut out = OpenOptions::new()
                .create(true)
                .write(true)
                .append(resumed)
                .truncate(!resumed)
                .open(&partial)
                .map_err(|e| failed(&e))?;
            let mut body = response.into_body().into_reader();
            let mut buffer = vec![0; 256 * 1024];
            loop {
                if asked.load(Ordering::SeqCst) != RUN {
                    return Err(Stopped::Asked);
                }
                let read = body.read(&mut buffer).map_err(|e| failed(&e))?;
                if read == 0 {
                    break;
                }
                out.write_all(&buffer[..read]).map_err(|e| failed(&e))?;
                progress();
            }
            out.sync_all().map_err(|e| failed(&e))?;
        }
        if sha256(&partial, asked)
            .map_err(|e| failed(&e))?
            .ok_or(Stopped::Asked)?
            != model.sha256
        {
            // A wrong file is never resumed: the next try starts again.
            let _ = fs::remove_file(&partial);
            return Err(Stopped::Failed(format!(
                "The {} downloaded damaged. Try again.",
                model.role
            )));
        }
        fs::rename(&partial, &file).map_err(|e| failed(&e))
    }

    /// Bytes of `model` on disk, installed or partial.
    fn on_disk(&self, model: &Model) -> u64 {
        if self.file(model).is_file() {
            return model.size;
        }
        fs::metadata(self.partial(model))
            .map(|m| m.len().min(model.size))
            .unwrap_or(0)
    }

    fn file(&self, model: &Model) -> PathBuf {
        self.folder.join(model.file_name)
    }

    fn partial(&self, model: &Model) -> PathBuf {
        self.folder.join(format!("{}.part", model.file_name))
    }

    fn delete_files(&self) -> io::Result<()> {
        for path in MODELS
            .iter()
            .flat_map(|model| [self.file(model), self.partial(model)])
        {
            match fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    fn send(&self, status: &Status) {
        // No window to tell is fine: the download goes on.
        let _ = self.app.emit(STATUS_EVENT, status);
    }
}

/// The file's SHA-256 in lower-case hex, or None if pause or cancel was
/// asked for while reading it.
fn sha256(path: &Path, asked: &AtomicU8) -> io::Result<Option<String>> {
    // A model file in the app's own data folder; no path comes from a web request.
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        if asked.load(Ordering::SeqCst) != RUN {
            return Ok(None);
        }
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Some(
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    ))
}

/// `bytes` as "1.8 GB", in the decimal units macOS and Windows show for disk space.
fn gigabytes(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}
