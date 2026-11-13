//! The Grouping Models (ADR-0004): two official GGUF files, pinned by
//! revision, size and SHA-256, downloaded on the user's say-so into a
//! folder in the app data folder. The download runs on its own thread, so
//! the user can scan and clean meanwhile.
//!
//! Each model downloads into `<file>.part` through `downloads`, the app's
//! only network access, resumed with an HTTP Range request after a pause or
//! an interruption, and is moved to `<file>` only once its SHA-256 matches.
//! So a model is installed exactly when `<file>` exists, and the state can
//! always be read from the folder.
//!
//! The user may also pick a model to describe files in place of the first
//! (ADR 0009), linked from Hugging Face. It downloads the same way into
//! `custom/`, beside a `source.json` saying where from, and is moved into
//! place only once it has described a sample file; its SHA-256 is shown,
//! never checked. At most one is kept, and the default stays on disk.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use files_cleaner_core::{self as core, SettingChange};
use files_cleaner_grouping as grouping;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::downloads;

/// The event the download's status is sent on, to every window.
pub const STATUS_EVENT: &str = "grouping-models";

/// How often, at most, the status is sent while downloading.
const STATUS_EVERY: Duration = Duration::from_millis(250);

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

/// The picked describing model's folder, inside the models folder.
const CUSTOM_FOLDER: &str = "custom";
/// Where the picked model comes from, inside its folder.
const SOURCE_FILE: &str = "source.json";

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
    /// The model the user picked to describe files, while it downloads or
    /// once it's in; `None` when there's none.
    pub custom: Option<CustomStatus>,
    /// Why the picked model's download stopped, or why it was refused.
    pub custom_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CustomState {
    Downloading,
    Paused,
    /// Downloaded, and being asked to describe the sample file.
    Checking,
    Installed,
}

/// The model the user picked, as Settings shows it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomStatus {
    pub state: CustomState,
    /// Its file's name.
    pub name: String,
    /// The Hugging Face link it was picked by.
    pub link: String,
    pub downloaded: u64,
    pub total: u64,
    /// Its SHA-256 in hex, once installed.
    pub sha256: Option<String>,
    /// Its file, or its partial file while downloading.
    pub path: Option<PathBuf>,
    /// Whether it describes files now, rather than the default.
    pub in_use: bool,
}

/// Where the picked model comes from, kept in `source.json` beside it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    link: String,
    url: String,
    file_name: String,
    size: u64,
    /// Set once it's checked and moved into place.
    sha256: Option<String>,
}

/// One file to download: where from, and where to.
struct Target<'a> {
    /// What it does, for error messages.
    role: &'a str,
    url: &'a str,
    size: u64,
    file: PathBuf,
    partial: PathBuf,
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
    /// The same, for the picked describing model's download.
    custom_running: Option<Arc<AtomicU8>>,
    /// Whether the picked model is being asked to describe the sample file.
    checking: bool,
    custom_error: Option<String>,
}

/// Why a download stopped before finishing.
enum Stopped {
    /// Pause or cancel was asked for.
    Asked,
    Failed(String),
    /// The picked model couldn't describe the sample file, for this reason.
    Refused(String),
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
    /// once both are installed. The card writer is the one the user picked,
    /// `describing_model` being its link, or the default when that's `None`.
    pub fn files(&self, describing_model: Option<&str>) -> Result<(PathBuf, PathBuf), String> {
        if !self.installed() {
            return Err("Download the grouping models first".to_string());
        }
        let describing = match describing_model {
            None => self.file(&MODELS[0]),
            Some(link) => self
                .source()
                .filter(|source| source.link == link)
                .map(|source| self.custom_file(&source))
                .filter(|file| file.is_file())
                .ok_or("The model you picked to describe files isn't downloaded. Pick it again in Settings, or use the default.")?,
        };
        Ok((describing, self.file(&MODELS[1])))
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
                    custom: None,
                    custom_error: None,
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
            custom: self.custom_status(inner),
            custom_error: inner.custom_error.clone(),
        }
    }

    fn custom_status(&self, inner: &Inner) -> Option<CustomStatus> {
        let source = self.source()?;
        let file = self.custom_file(&source);
        let partial = partial_of(&file);
        let installed = file.is_file();
        let state = match inner
            .custom_running
            .as_ref()
            .map(|asked| asked.load(Ordering::SeqCst))
        {
            // Its files go once its thread sees the cancel.
            Some(CANCEL) => return None,
            Some(_) if inner.checking => CustomState::Checking,
            Some(RUN) => CustomState::Downloading,
            Some(_) => CustomState::Paused,
            None if installed => CustomState::Installed,
            None => CustomState::Paused,
        };
        let downloaded = if installed {
            source.size
        } else {
            fs::metadata(&partial).map_or(0, |m| m.len().min(source.size))
        };
        // Unreadable settings are the default's.
        let in_use = installed
            && crate::read_config(&self.app)
                .is_ok_and(|loaded| loaded.config.describing_model.as_ref() == Some(&source.link));
        Some(CustomStatus {
            state,
            name: source.file_name,
            link: source.link,
            downloaded,
            total: source.size,
            sha256: source.sha256,
            path: [file, partial].into_iter().find(|path| path.is_file()),
            in_use,
        })
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
    /// every model file goes, the picked describing model's too.
    pub fn remove(&self) -> Result<Status, String> {
        // All under the lock, so a download can't start, or the running one end, in between.
        let mut inner = self.inner.lock().unwrap();
        inner.error = None;
        match &inner.running {
            Some(asked) => asked.store(CANCEL, Ordering::SeqCst),
            None => self.delete_files().map_err(|e| e.to_string())?,
        }
        self.remove_custom_locked(&mut inner)?;
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
                .try_for_each(|model| self.fetch_default(model, asked, &mut progress));
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
                (_, Err(Stopped::Failed(message) | Stopped::Refused(message))) => {
                    inner.error = Some(message)
                }
                _ => {}
            }
            inner.running = None;
            break self.status_of(&inner);
        };
        self.send(&status);
    }

    /// Downloads `model` into its partial file, from where that stopped,
    /// then moves it into place if its SHA-256 matches.
    fn fetch_default(
        &self,
        model: &Model,
        asked: &AtomicU8,
        progress: &mut impl FnMut(),
    ) -> Result<(), Stopped> {
        let target = Target {
            role: model.role,
            url: model.url,
            size: model.size,
            file: self.file(model),
            partial: self.partial(model),
        };
        if target.file.is_file() {
            return Ok(());
        }
        if fetch(&target, asked, progress)? != model.sha256 {
            // A wrong file is never resumed: the next try starts again.
            let _ = fs::remove_file(&target.partial);
            return Err(Stopped::Failed(format!(
                "The {} downloaded damaged. Try again.",
                model.role
            )));
        }
        fs::rename(&target.partial, &target.file)
            .map_err(|e| Stopped::Failed(format!("Downloading the {} failed: {e}", model.role)))
    }

    /// Starts downloading the describing model at the Hugging Face `link`,
    /// in place of any picked before, or resumes the one stopped when
    /// `link` is `None`. A link that isn't to one public `.gguf` file is
    /// refused, as is one the disk has no room for.
    pub fn download_custom(self: &Arc<Self>, link: Option<String>) -> Result<Status, String> {
        let source = match link {
            Some(link) => {
                if self.inner.lock().unwrap().custom_running.is_some() {
                    return Err("Cancel the model being downloaded first.".to_string());
                }
                let parsed = core::describing_model_link(&link)?;
                Source {
                    size: downloads::remote_size(&parsed.url)?,
                    link: link.trim().to_string(),
                    url: parsed.url,
                    file_name: parsed.file_name,
                    sha256: None,
                }
            }
            None => self
                .source()
                .ok_or("There's no model download to resume.")?,
        };
        let mut inner = self.inner.lock().unwrap();
        if let Some(asked) = &inner.custom_running {
            // Paused, but its thread hasn't noticed yet: it carries on.
            asked.store(RUN, Ordering::SeqCst);
        } else {
            let fresh = self
                .source()
                .is_none_or(|old| old.link != source.link || old.sha256.is_some());
            let partial = partial_of(&self.custom_file(&source));
            let left = match fresh {
                true => source.size,
                false => {
                    source.size - fs::metadata(&partial).map_or(0, |m| m.len().min(source.size))
                }
            };
            let needed = left + left / 10;
            let free = fs4::available_space(&self.folder).map_err(|e| e.to_string())?;
            if free < needed {
                return Err(format!(
                    "Not enough free space for {}: it needs {} and this disk has {} free.",
                    source.file_name,
                    gigabytes(needed),
                    gigabytes(free)
                ));
            }
            if fresh {
                // A new pick replaces the old one, and the default describes files meanwhile.
                self.remove_custom_locked(&mut inner)?;
                let folder = self.custom_folder();
                fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let json = serde_json::to_string(&source).map_err(|e| e.to_string())?;
                fs::write(folder.join(SOURCE_FILE), json).map_err(|e| e.to_string())?;
            }
            let asked = Arc::new(AtomicU8::new(RUN));
            inner.custom_running = Some(Arc::clone(&asked));
            inner.custom_error = None;
            let models = Arc::clone(self);
            std::thread::spawn(move || models.work_custom(&source, &asked));
        }
        let status = self.status_of(&inner);
        drop(inner);
        self.send(&status);
        Ok(status)
    }

    /// Pauses the picked model's download, keeping what's downloaded so far.
    pub fn pause_custom(&self) -> Status {
        let inner = self.inner.lock().unwrap();
        if let Some(asked) = &inner.custom_running {
            asked.store(PAUSE, Ordering::SeqCst);
        }
        let status = self.status_of(&inner);
        drop(inner);
        self.send(&status);
        status
    }

    /// Cancels the picked model's download, or removes it: the default
    /// describes files again.
    pub fn remove_custom(&self) -> Result<Status, String> {
        let mut inner = self.inner.lock().unwrap();
        self.remove_custom_locked(&mut inner)?;
        let status = self.status_of(&inner);
        drop(inner);
        self.send(&status);
        Ok(status)
    }

    /// Describes files with the picked model, once it's in, or with the
    /// default when `picked` is false. Nothing is downloaded either way.
    pub fn use_custom(&self, picked: bool) -> Result<Status, String> {
        let link = match picked {
            true => Some(
                self.source()
                    .filter(|source| source.sha256.is_some())
                    .ok_or("The model you picked isn't downloaded.")?
                    .link,
            ),
            false => None,
        };
        crate::save_setting(&self.app, &SettingChange::DescribingModel(link))
            .map_err(|e| e.message)?;
        let status = self.status();
        self.send(&status);
        Ok(status)
    }

    /// Cancels the picked model's download, or deletes it and goes back to
    /// the default; `inner` is held, so no download starts in between.
    fn remove_custom_locked(&self, inner: &mut Inner) -> Result<(), String> {
        inner.custom_error = None;
        if let Some(asked) = &inner.custom_running {
            asked.store(CANCEL, Ordering::SeqCst);
            return Ok(());
        }
        self.use_default_if_custom()?;
        self.delete_custom().map_err(|e| e.to_string())
    }

    /// Switches the setting back to the default model, if it names another.
    fn use_default_if_custom(&self) -> Result<(), String> {
        // Settings that can't be read can't name the picked model either.
        let picked = crate::read_config(&self.app)
            .is_ok_and(|loaded| loaded.config.describing_model.is_some());
        if picked {
            crate::save_setting(&self.app, &SettingChange::DescribingModel(None))
                .map_err(|e| e.message)?;
        }
        Ok(())
    }

    /// The thread downloading the picked model: fetches it, then has it
    /// describe the sample file, until it's in, refused, or it's paused,
    /// cancelled or fails.
    fn work_custom(&self, source: &Source, asked: &AtomicU8) {
        let mut last_sent = Instant::now();
        let mut progress = || {
            if last_sent.elapsed() >= STATUS_EVERY {
                last_sent = Instant::now();
                self.send(&self.status());
            }
        };
        let file = self.custom_file(source);
        let target = Target {
            role: "model you picked",
            url: &source.url,
            size: source.size,
            partial: partial_of(&file),
            file,
        };
        let status = loop {
            let result = fetch(&target, asked, &mut progress)
                .and_then(|sha256| self.accept_custom(source, &target, sha256, asked));
            // Under the lock, so a resume can't slip in between deciding to stop and stopping.
            let mut inner = self.inner.lock().unwrap();
            inner.checking = false;
            match (asked.load(Ordering::SeqCst), result) {
                (RUN, Err(Stopped::Asked)) => continue,
                (CANCEL, _) => {
                    // A cancel that came in as it was switched to goes back to the default.
                    let removed = self
                        .use_default_if_custom()
                        .and_then(|()| self.delete_custom().map_err(|e| e.to_string()));
                    if let Err(e) = removed {
                        inner.custom_error =
                            Some(format!("Couldn't delete the downloaded file: {e}"));
                    }
                }
                (_, Err(Stopped::Refused(reason))) => {
                    let _ = self.delete_custom();
                    inner.custom_error = Some(format!(
                        "{} can't be used: {reason}. It was deleted, and the default model still describes your files.",
                        source.file_name
                    ));
                }
                (_, Err(Stopped::Failed(message))) => inner.custom_error = Some(message),
                _ => {}
            }
            inner.custom_running = None;
            break self.status_of(&inner);
        };
        self.send(&status);
    }

    /// Has the downloaded picked model describe the sample file and, if it
    /// can, moves it into place with its SHA-256 and switches to it.
    fn accept_custom(
        &self,
        source: &Source,
        target: &Target,
        sha256: String,
        asked: &AtomicU8,
    ) -> Result<(), Stopped> {
        self.inner.lock().unwrap().checking = true;
        self.send(&self.status());
        grouping::check_describing_model(&target.partial).map_err(Stopped::Refused)?;
        if asked.load(Ordering::SeqCst) != RUN {
            return Err(Stopped::Asked);
        }
        let failed = |e: &dyn std::fmt::Display| {
            Stopped::Failed(format!("Couldn't switch to the model you picked: {e}"))
        };
        let checked = Source {
            sha256: Some(sha256),
            ..source.clone()
        };
        let json = serde_json::to_string(&checked).map_err(|e| failed(&e))?;
        fs::rename(&target.partial, &target.file).map_err(|e| failed(&e))?;
        fs::write(self.custom_folder().join(SOURCE_FILE), json).map_err(|e| failed(&e))?;
        crate::save_setting(
            &self.app,
            &SettingChange::DescribingModel(Some(source.link.clone())),
        )
        .map_err(|e| failed(&e.message))?;
        Ok(())
    }

    /// Where the picked model comes from, if there is one.
    fn source(&self) -> Option<Source> {
        let bytes = fs::read(self.custom_folder().join(SOURCE_FILE)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// The picked model's file. Its name starts with a hash of where it
    /// comes from, so the cards it writes, which are stored by its file
    /// name, are never those of another model with the same name.
    fn custom_file(&self, source: &Source) -> PathBuf {
        let from: String = Sha256::digest(source.url.as_bytes())[..4]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.custom_folder()
            .join(format!("{from}-{}", source.file_name))
    }

    fn custom_folder(&self) -> PathBuf {
        self.folder.join(CUSTOM_FOLDER)
    }

    fn delete_custom(&self) -> io::Result<()> {
        match fs::remove_dir_all(self.custom_folder()) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
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

/// Downloads `target` into its partial file, from where that stopped, and
/// returns its SHA-256; moving it into place is the caller's.
fn fetch(
    target: &Target,
    asked: &AtomicU8,
    progress: &mut impl FnMut(),
) -> Result<String, Stopped> {
    let failed = |e: &dyn std::fmt::Display| {
        Stopped::Failed(format!("Downloading the {} failed: {e}", target.role))
    };
    let stop = || asked.load(Ordering::SeqCst) != RUN;
    match downloads::fetch(target.url, target.size, &target.partial, &stop, progress) {
        Ok(downloads::Fetched::Complete) => {}
        Ok(downloads::Fetched::Stopped) => return Err(Stopped::Asked),
        Err(reason) => return Err(failed(&reason)),
    }
    sha256(&target.partial, asked)
        .map_err(|e| failed(&e))?
        .ok_or(Stopped::Asked)
}

/// The partial file `file` downloads into.
fn partial_of(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
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
