//! What needs llama.cpp, PDFium or Tesseract lives in this crate, so `core`
//! stays pure Rust. The user's Rules go first, by code alone; only the rest is
//! read by the models. `Analysis::regroup` runs no model, so settings can
//! change without reading again.

mod card;
mod cluster;
mod kinds;
mod llama;
mod marks;
mod ocr;
mod place;
mod read;
mod rules;
mod store;
mod trace;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use files_cleaner_core::{GroupBy, MeaningFolder, Rule, ScanEntry, ScanResult};
use serde::Serialize;

pub use card::Card;
pub use cluster::Vector;
pub use llama::{check_describing_model, LlamaModels};
pub use place::{Choice, Question};
pub use read::{pdf_child_main, read, Content};
pub use rules::overlaps as rule_overlaps;
pub use store::CardStore;
pub use trace::{clear_trace, trace_to};

use cluster::{centroid, cosine, normalized};
use place::Answer;
use store::{content_hash, ModelIds, Stored};

/// An interface, so the engine can be tested with fakes.
pub trait Models {
    /// Stored cards made by another model are not used.
    fn card_model_id(&self) -> String;
    /// Stored vectors made by another model are not used.
    fn embedding_model_id(&self) -> String;
    /// An error only means the file is grouped by its name alone.
    fn card(&mut self, content: &Content) -> Result<Card, String>;
    /// An error only leaves the file out.
    fn place(&mut self, question: &Question) -> Result<Option<usize>, String>;
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String>;
    fn device(&self) -> Option<String>;
    /// Apart from first use, so the wait can be told apart from reading. A
    /// failure comes back where the model is used.
    fn load(&mut self) {}
}

pub use files_cleaner_core::GroupingSettings as Settings;

pub struct Input<'a> {
    /// Without the copies trashed as duplicates.
    pub scan: &'a ScanResult,
    pub settings: Settings,
    pub store: &'a CardStore,
    /// Without it, PDFs are read by name.
    pub pdfium: Option<&'a Path>,
    /// Without it, scanned PDFs are read by name.
    pub tessdata: Option<&'a Path>,
    /// In order of priority.
    pub rules: &'a [Rule],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub finished: usize,
    pub total: usize,
    pub current: PathBuf,
    pub phase: Phase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// `total` counts every file.
    Checking,
    Loading,
    /// `total` counts only the files with no stored card.
    Reading,
    Placing,
}

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stopped {
    Cancelled,
    Failed(String),
}

impl fmt::Display for Stopped {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Stopped::Cancelled => write!(f, "The analysis was cancelled."),
            Stopped::Failed(e) => write!(f, "The grouping models couldn't compare the files: {e}"),
        }
    }
}

impl std::error::Error for Stopped {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grouping {
    pub folders: Vec<SuggestedFolder>,
    pub ungrouped: Vec<PathBuf>,
    pub kinds: BTreeMap<PathBuf, &'static str>,
    pub summaries: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedFolder {
    pub name: String,
    pub files: Vec<PathBuf>,
    pub existing: Option<PathBuf>,
    pub basis: Basis,
    /// The Rule that made it, for `Basis::Rule`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<Rule>,
}

/// Shown to the user as why the files are together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Basis {
    Topic,
    Kind,
    Existing,
    Rule,
}

pub struct Analysis {
    pub grouping: Grouping,
    /// `None` when every file's card was stored, so no model ran.
    pub device: Option<String>,
    files: Vec<PathBuf>,
    cards: Vec<Card>,
    vectors: Vec<Vector>,
    fixed: BTreeMap<&'static str, Vec<PathBuf>>,
    unread: Vec<PathBuf>,
    ruled: Vec<SuggestedFolder>,
    meaning: Vec<Meaning>,
    /// What files left out are compared with; `None` for folders they can't be
    /// placed in.
    rule_places: Vec<Option<(Vector, Vec<String>)>>,
    subfolders: Vec<Subfolder>,
    /// By prompt, so a regroup that asks the same question needs no model.
    answers: HashMap<String, Option<String>>,
}

struct Meaning {
    rule: Rule,
    files: Vec<usize>,
    folders: Vec<Named>,
}

struct Named {
    folder: MeaningFolder,
    /// Where it already exists on disk, if it does.
    existing: Option<PathBuf>,
    vector: Vector,
}

enum Fit {
    Folder(usize),
    /// Too close to call: the LLM picks one or none.
    Tie(Question, Vec<usize>),
    Stays,
}

const MEANING_THRESHOLD: f32 = 0.5;
/// Closer than this, embeddings can't tell folders apart, so the LLM decides.
const TIE_MARGIN: f32 = 0.03;

struct Subfolder {
    path: PathBuf,
    name: String,
    /// Of its sampled files, or of its name when there are none.
    centroid: Vector,
    topics: Vec<String>,
    files: Vec<String>,
}

#[derive(Clone, Copy)]
enum Target {
    Folder(usize),
    Subfolder(usize),
}

type Unanswered = (usize, String, Question);

const FOLDER_SAMPLE: usize = 5;
const EMBED_CHUNK: usize = 16;

/// Lets the user be asked before a long read. Every file is hashed to find out.
pub fn files_to_read(
    input: &Input,
    models: &dyn Models,
    cancel: &AtomicBool,
) -> Result<usize, Stopped> {
    let sorted = sort_out(input);
    let (card_id, embedding_id) = (models.card_model_id(), models.embedding_model_id());
    let ids = ModelIds {
        card: &card_id,
        embedding: &embedding_id,
    };
    let mut count = 0;
    for entry in files_to_check(&sorted.to_read, &sorted.samples) {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stopped::Cancelled);
        }
        let stored = content_hash(&entry.path).and_then(|h| input.store.get(&ids, &h));
        if stored.is_none() {
            count += 1;
        }
    }
    Ok(count)
}

struct SortedOut<'a> {
    ruled: Vec<SuggestedFolder>,
    by_meaning: Vec<rules::ByMeaning>,
    fixed: BTreeMap<&'static str, Vec<PathBuf>>,
    to_read: Vec<&'a ScanEntry>,
    samples: Vec<(&'a str, Vec<&'a ScanEntry>)>,
}

fn sort_out<'a>(input: &Input<'a>) -> SortedOut<'a> {
    let scan: &'a ScanResult = input.scan;
    let (ruled, by_meaning) =
        rules::apply(scan, input.rules, chrono::Local::now().date_naive(), true);
    let matched: HashSet<&Path> = ruled
        .iter()
        .flat_map(|f| &f.files)
        .map(PathBuf::as_path)
        .collect();
    let mut fixed: BTreeMap<&'static str, Vec<PathBuf>> = BTreeMap::new();
    let mut to_read: Vec<&ScanEntry> = Vec::new();
    let mut inside: BTreeMap<&str, Vec<&ScanEntry>> = BTreeMap::new();
    for entry in &scan.entries {
        // A file without an extension is never a Loose File nor a sample.
        if entry.is_package || entry.extension.is_empty() || matched.contains(entry.path.as_path())
        {
            continue;
        }
        let fixed_kind = kinds::fixed_kind(&entry.extension.to_lowercase());
        match (entry.relative_path.as_slice(), fixed_kind) {
            ([_], Some(kind)) => fixed.entry(kind).or_default().push(entry.path.clone()),
            ([_], None) => to_read.push(entry),
            ([folder, ..], None) => inside.entry(folder.as_str()).or_default().push(entry),
            // A subfolder holding only unread kinds is still a destination.
            ([folder, ..], Some(_)) => {
                inside.entry(folder.as_str()).or_default();
            }
            ([], _) => {}
        }
    }
    // The files nearest each subfolder's top, so a deep tree doesn't crowd it out.
    let samples: Vec<(&str, Vec<&ScanEntry>)> = inside
        .into_iter()
        .map(|(folder, mut entries)| {
            entries.sort_by(|a, b| {
                (a.relative_path.len(), &a.relative_path)
                    .cmp(&(b.relative_path.len(), &b.relative_path))
            });
            entries.truncate(FOLDER_SAMPLE);
            (folder, entries)
        })
        .collect();

    SortedOut {
        ruled,
        by_meaning,
        fixed,
        to_read,
        samples,
    }
}

fn files_to_check<'a>(
    to_read: &[&'a ScanEntry],
    samples: &[(&'a str, Vec<&'a ScanEntry>)],
) -> Vec<&'a ScanEntry> {
    to_read
        .iter()
        .copied()
        .chain(samples.iter().flat_map(|(_, s)| s.iter().copied()))
        .collect()
}

/// Cards are stored as they're made, so a cancelled analysis keeps them. `stop`
/// differs from `cancel`: the files read so far are still grouped.
pub fn analyse(
    input: &Input,
    models: &mut dyn Models,
    cancel: &AtomicBool,
    stop: &AtomicBool,
    mut on_progress: impl FnMut(Progress),
) -> Result<Analysis, Stopped> {
    let SortedOut {
        ruled,
        by_meaning,
        mut fixed,
        mut to_read,
        mut samples,
    } = sort_out(input);
    let scan = input.scan;
    let (card_id, embedding_id) = (models.card_model_id(), models.embedding_model_id());
    let ids = ModelIds {
        card: &card_id,
        embedding: &embedding_id,
    };
    let mut all = files_to_check(&to_read, &samples);
    // The stored cards first, so the files that need reading are counted up front.
    let mut stored: Vec<(Option<blake3::Hash>, Option<Stored>)> = Vec::with_capacity(all.len());
    let mut last_report: Option<Instant> = None;
    for (checked, entry) in all.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stopped::Cancelled);
        }
        if last_report.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL) {
            on_progress(Progress {
                finished: checked,
                total: all.len(),
                current: entry.path.clone(),
                phase: Phase::Checking,
            });
            last_report = Some(Instant::now());
        }
        let hash = content_hash(&entry.path);
        let found = hash.as_ref().and_then(|h| input.store.get(&ids, h));
        stored.push((hash, found));
    }
    let total = stored.iter().filter(|(_, found)| found.is_none()).count();
    if let Some((first, _)) = all
        .iter()
        .zip(&stored)
        .find(|(_, (_, found))| found.is_none())
        .filter(|_| !stop.load(Ordering::Relaxed))
    {
        on_progress(Progress {
            finished: 0,
            total,
            current: first.path.clone(),
            phase: Phase::Loading,
        });
        models.load();
    }
    let mut finished = 0;
    let mut last_report: Option<Instant> = None;
    for (entry, (hash, found)) in all.iter().zip(stored.iter_mut()) {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stopped::Cancelled);
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let name = &entry.name;
        if let Some(found) = found {
            trace::time(&format!("{name}: saved card\n"));
            if trace::enabled() {
                trace::write(
                    trace::CARDS,
                    &format!("{name}.txt"),
                    &format!(
                        "(A card saved by an earlier analysis: the prompt wasn't run again. Forget the saved cards to see it.)\n\n\
                         === CARD ===\n{:#?}\n",
                        found.card
                    ),
                );
            }
            continue;
        }
        if last_report.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL) {
            on_progress(Progress {
                finished,
                total,
                current: entry.path.clone(),
                phase: Phase::Reading,
            });
            last_report = Some(Instant::now());
        }
        let started = Instant::now();
        trace::time(&format!("{name}\n"));
        let content = read::read(&entry.path, &entry.name, input.pdfium, input.tessdata);
        let read_time = started.elapsed();
        let card = if content.scanned {
            Card::default()
        } else {
            models.card(&content).unwrap_or_default()
        };
        trace::time(&format!(
            "  read {} · card {} · total {}{}\n",
            trace::secs(read_time),
            if content.scanned {
                "skipped (scanned)".to_string()
            } else {
                trace::secs(started.elapsed() - read_time)
            },
            trace::secs(started.elapsed()),
            if content.body.is_empty() {
                " · by name only"
            } else {
                ""
            }
        ));
        let made = Stored {
            card,
            scanned: content.scanned,
            embedding: None,
        };
        if let Some(h) = &hash {
            input.store.put(&ids, h, &made);
        }
        *found = Some(made);
        finished += 1;
    }
    if let Some(last) = all.last().filter(|_| total > 0) {
        on_progress(Progress {
            finished,
            total,
            current: last.path.clone(),
            phase: Phase::Reading,
        });
    }
    // `all` is `to_read`, then each subfolder's sample; flags line up with it.
    let read: Vec<bool> = stored.iter().map(|(_, found)| found.is_some()).collect();
    let mut unread = Vec::new();
    let mut flags = read.iter();
    to_read.retain(|entry| {
        let was_read = *flags.next().unwrap();
        if !was_read {
            unread.push(entry.path.clone());
        }
        was_read
    });
    for (_, sample) in &mut samples {
        sample.retain(|_| *flags.next().unwrap());
    }
    let mut flags = read.iter();
    all.retain(|_| *flags.next().unwrap());
    let mut stored: Vec<(Option<blake3::Hash>, Stored)> = stored
        .into_iter()
        .filter_map(|(hash, found)| Some((hash, found?)))
        .collect();
    if cancel.load(Ordering::Relaxed) {
        return Err(Stopped::Cancelled);
    }

    // A sampled one still stands for its subfolder by name.
    let scanned: Vec<bool> = stored
        .iter()
        .enumerate()
        .map(|(i, (_, s))| i < to_read.len() && s.scanned)
        .collect();
    for (entry, _) in to_read.iter().zip(&scanned).filter(|(_, &s)| s) {
        fixed
            .entry(kinds::SCANNED_PDFS)
            .or_default()
            .push(entry.path.clone());
    }
    let mut flags = scanned.iter();
    to_read.retain(|_| !flags.next().unwrap());
    let mut flags = scanned.iter();
    all.retain(|_| !flags.next().unwrap());
    let mut flags = scanned.iter();
    stored.retain(|_| !flags.next().unwrap());

    // All cards before any embedding, then every missing text is embedded
    // together.
    let missing: Vec<usize> = (0..stored.len())
        .filter(|&i| stored[i].1.embedding.is_none())
        .collect();
    let unsampled: Vec<&str> = samples
        .iter()
        .filter(|(_, s)| s.is_empty())
        .map(|(f, _)| *f)
        .collect();
    let offered_names: Vec<String> = ruled
        .iter()
        .filter(|f| offered(f))
        .map(|f| f.name.replace('/', " "))
        .collect();
    let texts: Vec<String> = missing
        .iter()
        .map(|&i| stored[i].1.card.text(&read::name_words(&all[i].name)))
        .chain(unsampled.iter().map(|f| f.to_string()))
        .chain(offered_names.iter().cloned())
        .chain(
            by_meaning
                .iter()
                .flat_map(|taken| named_folders(&taken.rule))
                .map(folder_text),
        )
        .collect();
    let mut vectors = Vec::with_capacity(texts.len());
    let started = Instant::now();
    for chunk in texts.chunks(EMBED_CHUNK) {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stopped::Cancelled);
        }
        vectors.extend(
            models
                .embed(chunk)
                .map_err(Stopped::Failed)?
                .into_iter()
                .map(normalized),
        );
    }
    trace::time(&format!(
        "\nembedded {} texts, {}\n\n",
        texts.len(),
        trace::secs(started.elapsed())
    ));
    let mut name_vectors = vectors.split_off(missing.len());
    let mut rule_vectors = name_vectors.split_off(unsampled.len());
    let meaning_vectors = rule_vectors.split_off(offered_names.len());
    for (&i, vector) in missing.iter().zip(vectors) {
        let (hash, entry) = &mut stored[i];
        entry.embedding = Some(vector);
        if let Some(h) = hash {
            input.store.put(&ids, h, entry);
        }
    }

    let mut stored = stored
        .into_iter()
        .map(|(_, s)| (s.card, s.embedding.unwrap_or_default()));
    let (cards, vectors): (Vec<Card>, Vec<Vector>) = stored.by_ref().take(to_read.len()).unzip();
    let mut name_vectors = name_vectors.into_iter();
    let subfolders: Vec<Subfolder> = samples
        .iter()
        .filter_map(|(folder, sample)| {
            let sampled: Vec<(Card, Vector)> = stored.by_ref().take(sample.len()).collect();
            let centroid = if sample.is_empty() {
                name_vectors.next()
            } else {
                centroid(sampled.iter().map(|(_, v)| v))
            }?;
            let topics = distinct(sampled.iter().map(|(c, _)| c.topic.as_str()));
            let files = sample.iter().map(|e| e.name.clone()).collect();
            Some(Subfolder {
                path: scan.root.join(folder),
                name: folder.to_string(),
                centroid,
                topics,
                files,
            })
        })
        .collect();
    let mut rule_vectors = rule_vectors.into_iter();
    let rule_places = ruled
        .iter()
        .map(|folder| {
            if !offered(folder) {
                return None;
            }
            let named = rule_vectors.next()?;
            let subfolder = folder
                .existing
                .as_ref()
                .and_then(|path| subfolders.iter().find(|s| &s.path == path));
            Some(match subfolder {
                Some(s) => (s.centroid.clone(), s.topics.clone()),
                None => (named, Vec::new()),
            })
        })
        .collect();
    let read_at: HashMap<&Path, usize> = to_read
        .iter()
        .enumerate()
        .map(|(i, e)| (e.path.as_path(), i))
        .collect();
    let mut meaning_vectors = meaning_vectors.into_iter();
    let meaning = by_meaning
        .into_iter()
        .map(|taken| {
            let folders = named_folders(&taken.rule)
                .iter()
                .zip(meaning_vectors.by_ref())
                .map(|(folder, vector)| Named {
                    existing: rules::existing_folder(&scan.root, folder.name.trim()),
                    folder: folder.clone(),
                    vector,
                })
                .collect();
            // There is nothing to sort it by.
            let files = taken
                .files
                .iter()
                .filter_map(|p| read_at.get(p.as_path()).copied())
                .collect();
            Meaning {
                rule: taken.rule,
                files,
                folders,
            }
        })
        .collect();
    let mut analysis = Analysis {
        grouping: Grouping {
            folders: Vec::new(),
            ungrouped: Vec::new(),
            kinds: BTreeMap::new(),
            summaries: BTreeMap::new(),
        },
        device: None,
        files: to_read.iter().map(|e| e.path.clone()).collect(),
        cards,
        vectors,
        fixed,
        unread,
        ruled,
        meaning,
        rule_places,
        subfolders,
        answers: HashMap::new(),
    };

    // Placing a file changes no other file's question, so one round asks them all.
    let (_, unanswered) = analysis.grouped(&input.settings);
    let mut last_report: Option<Instant> = None;
    for (finished, (file, prompt, question)) in unanswered.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stopped::Cancelled);
        }
        if last_report.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL) {
            let current = analysis.files[*file].clone();
            on_progress(Progress {
                finished,
                total: unanswered.len(),
                current,
                phase: Phase::Placing,
            });
            last_report = Some(Instant::now());
        }
        let started = Instant::now();
        let name = &question.file_name;
        let answer = match input.store.get_answer(&card_id, prompt) {
            // The user is waiting, so the model isn't asked: the file stays
            // where it is.
            None if stop.load(Ordering::Relaxed) => continue,
            Some(saved) => {
                trace::time(&format!(
                    "placing {name}: saved answer, {}\n",
                    trace::secs(started.elapsed())
                ));
                if trace::enabled() {
                    let folder = saved.folder.as_deref().unwrap_or("none");
                    trace::write(
                        trace::PLACING,
                        &format!("{name}.txt"),
                        &format!("(An answer saved by an earlier analysis: the model wasn't asked again.)\n\n\
                                  === PROMPT ===\n{prompt}\n=== FOLDER ===\n{folder}\n"),
                    );
                }
                saved
            }
            None => {
                let chosen = models.place(question);
                let answer = Answer {
                    folder: chosen
                        .as_ref()
                        .ok()
                        .copied()
                        .flatten()
                        .map(|i| place::label(&question.folders[i])),
                };
                // A failed question is asked again next time.
                if chosen.is_ok() {
                    input.store.put_answer(&card_id, prompt, &answer);
                }
                trace::time(&format!(
                    "placing {name}: asked, {}\n",
                    trace::secs(started.elapsed())
                ));
                answer
            }
        };
        analysis.answers.insert(prompt.clone(), answer.folder);
    }
    analysis.device = models.device();
    analysis.grouping = analysis.regroup(&input.settings);
    Ok(analysis)
}

fn named_folders(rule: &Rule) -> &[MeaningFolder] {
    match &rule.group_by {
        GroupBy::Meaning { folders } => folders,
        _ => &[],
    }
}

fn folder_text(folder: &MeaningFolder) -> String {
    match folder.description.as_deref().map(str::trim) {
        Some(about) if !about.is_empty() => format!("{}. {about}", folder.name.trim()),
        _ => folder.name.trim().to_string(),
    }
}

/// Only a Rule's single named folder: grouping by date or kind has nothing to
/// do with what a file is about.
fn offered(folder: &SuggestedFolder) -> bool {
    matches!(
        folder.rule.as_ref().map(|rule| &rule.group_by),
        Some(GroupBy::Folder { .. })
    )
}

/// `by_meaning` holds back files that wait for the models; without them those
/// Rules are passed over.
pub fn group_by_rules(scan: &ScanResult, rules: &[Rule], by_meaning: bool) -> Grouping {
    let (mut folders, _) = rules::apply(scan, rules, chrono::Local::now().date_naive(), by_meaning);
    let matched: HashSet<&PathBuf> = folders.iter().flat_map(|f| &f.files).collect();
    let mut ungrouped: Vec<PathBuf> = scan
        .entries
        .iter()
        .filter(|e| rules::is_loose(e) && !matched.contains(&e.path))
        .map(|e| e.path.clone())
        .collect();
    ungrouped.sort();
    folders.sort_by(|a, b| {
        b.files
            .len()
            .cmp(&a.files.len())
            .then_with(|| a.name.cmp(&b.name))
    });
    let kinds = folders
        .iter()
        .flat_map(|f| &f.files)
        .chain(&ungrouped)
        .map(|p| (p.clone(), kinds::kind(&extension(p))))
        .collect();
    Grouping {
        folders,
        ungrouped,
        kinds,
        summaries: BTreeMap::new(),
    }
}

impl Analysis {
    /// Runs no model, so it can follow a settings change at once.
    pub fn regroup(&self, settings: &Settings) -> Grouping {
        self.grouped(settings).0
    }

    fn grouped(&self, settings: &Settings) -> (Grouping, Vec<Unanswered>) {
        let folder_names: Vec<String> = self.subfolders.iter().map(|f| f.name.clone()).collect();
        // A Rule's folders come first: no other folder takes their names.
        let mut folders = self.ruled.clone();
        let mut ungrouped = Vec::new();
        let mut used_names: HashSet<String> =
            folders.iter().map(|f| f.name.to_lowercase()).collect();

        let mut unanswered = Vec::new();

        let topics: Vec<Option<String>> = self.cards.iter().map(Card::topic_key).collect();
        let same_topic = |i: usize, j: usize| self.same_topic(&topics, i, j);
        // Then each Rule by meaning's folders, before any other takes their names.
        for meaning in &self.meaning {
            self.sort_by_meaning(
                meaning,
                &topics,
                settings,
                &mut folders,
                &mut used_names,
                &mut ungrouped,
                &mut unanswered,
            );
        }
        let taken: HashSet<usize> = self
            .meaning
            .iter()
            .flat_map(|m| &m.files)
            .copied()
            .collect();
        let rest: Vec<usize> = (0..self.files.len())
            .filter(|i| !taken.contains(i))
            .collect();
        // A folder nobody can name isn't useful, so its files are placed one by
        // one instead.
        let (groups, left_out) = self.topic_groups(&rest, &topics, settings);
        let mut named: Vec<(String, &[usize])> = Vec::new();
        let mut topical: Vec<(usize, Vector, Vec<String>)> = self
            .rule_places
            .iter()
            .enumerate()
            .filter_map(|(k, place)| {
                let (centre, topics) = place.as_ref()?;
                Some((k, centre.clone(), topics.clone()))
            })
            .collect();

        // Never read, so only its name can tie it to a group:
        // `Lab5_StudentID.zip` with `Lab5_Report.docx`.
        let group_marks: Vec<HashSet<String>> = groups
            .iter()
            .map(|(members, _)| {
                members
                    .iter()
                    .flat_map(|&i| marks::name_marks(&file_name(&self.files[i])))
                    .collect()
            })
            .collect();
        let mut joined: Vec<Vec<PathBuf>> = vec![Vec::new(); groups.len()];
        let mut fixed: BTreeMap<&str, Vec<PathBuf>> = BTreeMap::new();
        for (kind, files) in &self.fixed {
            for file in files {
                let own = marks::name_marks(&file_name(file));
                match group_marks
                    .iter()
                    .position(|g| own.iter().any(|m| g.contains(m)))
                {
                    Some(g) => joined[g].push(file.clone()),
                    None => fixed.entry(kind).or_default().push(file.clone()),
                }
            }
        }

        for (kind, files) in fixed {
            if files.len() < settings.smallest_folder {
                ungrouped.extend(files);
                continue;
            }
            // A Rule's folder may already have the kind's name.
            let name = unused_name(&mut used_names, kind);
            let existing = kinds::matching_folder(kind, &folder_names)
                .and_then(|name| self.subfolders.iter().find(|f| f.name == name))
                .map(|f| f.path.clone())
                .filter(|path| !self.ruled.iter().any(|r| r.existing.as_ref() == Some(path)));
            folders.push(SuggestedFolder {
                name,
                files,
                existing,
                basis: Basis::Kind,
                rule: None,
            });
        }

        for ((members, topic), joined) in groups.iter().zip(joined) {
            let centre = centroid(members.iter().map(|&i| &self.vectors[i])).unwrap_or_default();
            let existing = self
                .subfolders
                .iter()
                .map(|f| (f, cosine(&centre, &f.centroid)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .filter(|(_, s)| *s >= settings.existing_folder_threshold)
                .map(|(f, _)| f.path.clone());
            let member_topics: Vec<&str> = members
                .iter()
                .map(|&i| self.cards[i].topic.trim())
                .filter(|t| !t.is_empty())
                .collect();
            let name = unused_name(&mut used_names, topic);
            named.push((name.clone(), members));
            topical.push((
                folders.len(),
                centre,
                distinct(member_topics.iter().copied()),
            ));
            let files = members
                .iter()
                .map(|&i| self.files[i].clone())
                .chain(joined)
                .collect();
            folders.push(SuggestedFolder {
                name,
                files,
                existing,
                basis: Basis::Topic,
                rule: None,
            });
        }

        // Every question is put before any file is placed, so none depends on another's answer.
        let questions: Vec<(usize, String, Question, Vec<Target>)> = left_out
            .iter()
            .filter_map(|&i| {
                let (question, targets) = self.question(i, &folders, &topical)?;
                Some((i, place::prompt(&question), question, targets))
            })
            .collect();
        let mut placed: BTreeMap<usize, String> = BTreeMap::new();
        let mut made: HashMap<usize, usize> = HashMap::new();
        for (i, prompt, question, targets) in questions {
            let Some(answer) = self.answers.get(&prompt) else {
                unanswered.push((i, prompt, question));
                continue;
            };
            let Some(c) = answer.as_ref().and_then(|name| {
                question
                    .folders
                    .iter()
                    .position(|f| &place::label(f) == name)
            }) else {
                continue;
            };
            let folder = match targets[c] {
                Target::Folder(k) => k,
                Target::Subfolder(s) => *made.entry(s).or_insert_with(|| {
                    let subfolder = &self.subfolders[s];
                    let name = unused_name(&mut used_names, &subfolder.name);
                    folders.push(SuggestedFolder {
                        name,
                        files: Vec::new(),
                        existing: Some(subfolder.path.clone()),
                        basis: Basis::Existing,
                        rule: None,
                    });
                    folders.len() - 1
                }),
            };
            folders[folder].files.push(self.files[i].clone());
            placed.insert(i, folders[folder].name.clone());
        }
        ungrouped.extend(
            left_out
                .iter()
                .filter(|i| !placed.contains_key(i))
                .map(|&i| self.files[i].clone()),
        );
        ungrouped.extend(self.unread.iter().cloned());
        if trace::enabled() {
            let similarity = |i: usize, j: usize| {
                cluster::similarity(&self.vectors[i], &self.vectors[j], same_topic(i, j))
            };
            let report = self.similarity_report(settings, &named, &left_out, &placed, similarity);
            trace::write(trace::SIMILARITY, "similarity.txt", &report);
        }
        folders.sort_by(|a, b| {
            b.files
                .len()
                .cmp(&a.files.len())
                .then_with(|| a.name.cmp(&b.name))
        });
        ungrouped.sort();
        let kinds = folders
            .iter()
            .flat_map(|f| &f.files)
            .chain(&ungrouped)
            .map(|p| (p.clone(), kinds::kind(&extension(p))))
            .collect();
        let summaries = self
            .files
            .iter()
            .zip(&self.cards)
            .filter(|(_, card)| !card.summary.is_empty())
            .map(|(path, card)| (path.clone(), card.summary.clone()))
            .collect();
        (
            Grouping {
                folders,
                ungrouped,
                kinds,
                summaries,
            },
            unanswered,
        )
    }

    /// The LLM names topics too narrowly, so a shared mark is enough: "IELTS
    /// Speaking" and "IELTS Writing".
    fn same_topic(&self, topics: &[Option<String>], i: usize, j: usize) -> bool {
        match (&topics[i], &topics[j]) {
            (Some(a), Some(b)) => {
                a == b || marks::share_mark(&self.cards[i].topic, &self.cards[j].topic)
            }
            _ => false,
        }
    }

    fn topic_groups(
        &self,
        members: &[usize],
        topics: &[Option<String>],
        settings: &Settings,
    ) -> (Vec<(Vec<usize>, String)>, Vec<usize>) {
        let vectors: Vec<Vector> = members.iter().map(|&i| self.vectors[i].clone()).collect();
        let clusters = cluster::cluster(
            &vectors,
            |a, b| self.same_topic(topics, members[a], members[b]),
            settings.folder_threshold,
            settings.smallest_folder,
        );
        let mut groups = Vec::new();
        let mut left_out: Vec<usize> = clusters.ungrouped.iter().map(|&a| members[a]).collect();
        for group in clusters.groups {
            let group: Vec<usize> = group.into_iter().map(|a| members[a]).collect();
            match self.shared_topic(&group, topics) {
                Some(topic) => groups.push((group, topic)),
                None => left_out.extend(group),
            }
        }
        (groups, left_out)
    }

    #[allow(clippy::too_many_arguments)]
    fn sort_by_meaning(
        &self,
        meaning: &Meaning,
        topics: &[Option<String>],
        settings: &Settings,
        folders: &mut Vec<SuggestedFolder>,
        used_names: &mut HashSet<String>,
        ungrouped: &mut Vec<PathBuf>,
        unanswered: &mut Vec<Unanswered>,
    ) {
        let made = |name: String, files: Vec<PathBuf>, existing: Option<PathBuf>| SuggestedFolder {
            name,
            files,
            existing,
            basis: Basis::Rule,
            rule: Some(meaning.rule.clone()),
        };
        if meaning.folders.is_empty() {
            let (groups, left_out) = self.topic_groups(&meaning.files, topics, settings);
            for (members, topic) in groups {
                let name = unused_name(used_names, &topic);
                let existing = self
                    .subfolders
                    .iter()
                    .find(|f| f.name.eq_ignore_ascii_case(&name))
                    .map(|f| f.path.clone());
                let files = members.iter().map(|&i| self.files[i].clone()).collect();
                folders.push(made(name, files, existing));
            }
            ungrouped.extend(left_out.iter().map(|&i| self.files[i].clone()));
            return;
        }
        let mut filled: Vec<Vec<PathBuf>> = vec![Vec::new(); meaning.folders.len()];
        for &i in &meaning.files {
            let chosen = match self.meaning_fit(i, meaning) {
                Fit::Folder(k) => Some(k),
                Fit::Stays => None,
                Fit::Tie(question, places) => {
                    let prompt = place::prompt(&question);
                    match self.answers.get(&prompt) {
                        None => {
                            unanswered.push((i, prompt, question));
                            None
                        }
                        Some(answer) => answer.as_ref().and_then(|name| {
                            let c = question
                                .folders
                                .iter()
                                .position(|f| &place::label(f) == name)?;
                            Some(places[c])
                        }),
                    }
                }
            };
            match chosen {
                Some(k) => filled[k].push(self.files[i].clone()),
                None => ungrouped.push(self.files[i].clone()),
            }
        }
        for (named, files) in meaning.folders.iter().zip(filled) {
            if files.is_empty() {
                continue;
            }
            let name = named.folder.name.trim();
            // Two Rules naming one folder mean the same folder.
            match folders
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
            {
                Some(k) => folders[k].files.extend(files),
                None => {
                    used_names.insert(name.to_lowercase());
                    folders.push(made(name.to_string(), files, named.existing.clone()));
                }
            }
        }
    }

    /// An empty card leaves nothing to ask about, so a tie with one stays.
    fn meaning_fit(&self, i: usize, meaning: &Meaning) -> Fit {
        let mut scores: Vec<(f32, usize)> = meaning
            .folders
            .iter()
            .enumerate()
            .map(|(k, named)| (cosine(&self.vectors[i], &named.vector), k))
            .collect();
        scores.sort_by(|a, b| b.0.total_cmp(&a.0));
        let Some(&(top, best)) = scores.first().filter(|(s, _)| *s >= MEANING_THRESHOLD) else {
            return Fit::Stays;
        };
        let close: Vec<usize> = scores
            .iter()
            .take_while(|(s, _)| top - s < TIE_MARGIN)
            .take(place::CHOICES)
            .map(|&(_, k)| k)
            .collect();
        if close.len() == 1 {
            return Fit::Folder(best);
        }
        let card = &self.cards[i];
        if card.topic.is_empty() && card.summary.is_empty() {
            return Fit::Stays;
        }
        let choices = close
            .iter()
            .map(|&k| {
                let folder = &meaning.folders[k].folder;
                Choice {
                    name: folder.name.trim().to_string(),
                    topics: folder.description.iter().cloned().collect(),
                    files: Vec::new(),
                    existing: false,
                }
            })
            .collect();
        let question = Question {
            file_name: file_name(&self.files[i]),
            card: card.clone(),
            folders: choices,
        };
        Fit::Tie(question, close)
    }

    /// `None` when there is nothing to ask about or nowhere to go.
    fn question(
        &self,
        i: usize,
        folders: &[SuggestedFolder],
        topical: &[(usize, Vector, Vec<String>)],
    ) -> Option<(Question, Vec<Target>)> {
        let card = &self.cards[i];
        if card.topic.is_empty() && card.summary.is_empty() {
            return None;
        }
        let mut choices: Vec<(f32, Choice, Target)> = topical
            .iter()
            .map(|(k, centre, topics)| {
                let files = folders[*k].files.iter().map(|p| file_name(p)).collect();
                let choice = Choice {
                    name: folders[*k].name.clone(),
                    topics: topics.clone(),
                    files,
                    existing: false,
                };
                (cosine(&self.vectors[i], centre), choice, Target::Folder(*k))
            })
            .collect();
        let taken: HashSet<&PathBuf> = folders.iter().filter_map(|f| f.existing.as_ref()).collect();
        choices.extend(
            self.subfolders
                .iter()
                .enumerate()
                .filter(|(_, f)| !taken.contains(&f.path))
                .map(|(s, f)| {
                    let choice = Choice {
                        name: f.name.clone(),
                        topics: f.topics.clone(),
                        files: f.files.clone(),
                        existing: true,
                    };
                    (
                        cosine(&self.vectors[i], &f.centroid),
                        choice,
                        Target::Subfolder(s),
                    )
                }),
        );
        choices.sort_by(|a, b| b.0.total_cmp(&a.0));
        choices.truncate(place::CHOICES);
        if choices.is_empty() {
            return None;
        }
        let (choices, targets): (Vec<Choice>, Vec<Target>) =
            choices.into_iter().map(|(_, c, t)| (c, t)).unzip();
        Some((
            Question {
                file_name: file_name(&self.files[i]),
                card: card.clone(),
                folders: choices,
            },
            targets,
        ))
    }

    /// For tuning the thresholds: compares what clustering compares.
    fn similarity_report(
        &self,
        settings: &Settings,
        folders: &[(String, &[usize])],
        ungrouped: &[usize],
        placed: &BTreeMap<usize, String>,
        similarity: impl Fn(usize, usize) -> f32,
    ) -> String {
        const NEAREST: usize = 10;
        let n = self.files.len();
        let mut home: Vec<Option<&str>> = vec![None; n];
        for (name, members) in folders {
            members.iter().for_each(|&i| home[i] = Some(name));
        }
        let label = |i: usize| {
            format!(
                "{}  [topic: {}]",
                file_name(&self.files[i]),
                self.cards[i].topic
            )
        };
        let average = |i: usize, members: &[usize]| {
            let others: Vec<f32> = members
                .iter()
                .filter(|&&j| j != i)
                .map(|&j| similarity(i, j))
                .collect();
            others.iter().sum::<f32>() / others.len().max(1) as f32
        };
        let mut out = String::new();
        let _ = writeln!(
            out,
            "Folder threshold {:.2} · existing folder threshold {:.2} · smallest folder {}\n\
             Same topic, or topics sharing a mark, count as 1.00. Folders of archives, images, installers and scanned PDFs aren't listed.\n",
            settings.folder_threshold, settings.existing_folder_threshold, settings.smallest_folder
        );
        for (name, members) in folders {
            let inside =
                members.iter().map(|&i| average(i, members)).sum::<f32>() / members.len() as f32;
            let _ = writeln!(
                out,
                "== {name} ({} files), average inside {inside:.2}",
                members.len()
            );
            for &i in members.iter() {
                let _ = writeln!(out, "   in   {:.2}  {}", average(i, members), label(i));
            }
            let mut outside: Vec<(f32, usize)> = (0..n)
                .filter(|i| !members.contains(i))
                .map(|i| (average(i, members), i))
                .collect();
            outside.sort_by(|a, b| b.0.total_cmp(&a.0));
            for &(score, i) in outside.iter().take(NEAREST) {
                let place = home[i].map_or("left where it is".to_string(), |f| format!("in {f}"));
                let _ = writeln!(out, "   out  {score:.2}  {}  ({place})", label(i));
            }
            let centre = centroid(members.iter().map(|&i| &self.vectors[i])).unwrap_or_default();
            for f in &self.subfolders {
                let _ = writeln!(
                    out,
                    "   existing {:.2}  {}/",
                    cosine(&centre, &f.centroid),
                    f.name
                );
            }
            out.push('\n');
        }
        let _ = writeln!(out, "== Left out of every folder");
        for &i in ungrouped {
            let nearest_file = (0..n)
                .filter(|&j| j != i)
                .map(|j| (similarity(i, j), j))
                .max_by(|a, b| a.0.total_cmp(&b.0));
            let nearest_folder = folders
                .iter()
                .map(|(name, members)| (average(i, members), name))
                .max_by(|a, b| a.0.total_cmp(&b.0));
            let _ = write!(out, "   {}", label(i));
            if let Some((score, name)) = nearest_folder {
                let _ = write!(out, "\n        nearest folder {score:.2} {name}");
            }
            if let Some((score, j)) = nearest_file {
                let _ = write!(
                    out,
                    "\n        nearest file   {score:.2} {}",
                    file_name(&self.files[j])
                );
            }
            if let Some(folder) = placed.get(&i) {
                let _ = write!(out, "\n        placed by the model in {folder}");
            }
            out.push('\n');
        }
        out
    }

    /// A shared mark is preferred over the commonest topic, as topics are often
    /// too narrow ("IELTS" over "IELTS Writing").
    fn shared_topic(&self, members: &[usize], topics: &[Option<String>]) -> Option<String> {
        let enough = |n: usize| n >= 2 && n * 2 >= members.len();
        let member_topics: Vec<&str> = members
            .iter()
            .map(|&i| self.cards[i].topic.trim())
            .filter(|t| !t.is_empty())
            .collect();
        let agreed = members
            .iter()
            .filter_map(|&i| topics[i].as_ref())
            .collect::<HashSet<_>>()
            .len()
            <= 1;
        let mark = (!agreed && enough(member_topics.len()))
            .then(|| marks::shared_mark(&member_topics))
            .flatten()
            .map(str::to_string);
        mark.or_else(|| {
            most_common(member_topics.iter().copied())
                .filter(|(_, n)| enough(*n))
                .map(|(topic, _)| topic)
        })
        .map(|topic| capitalized(&topic))
    }
}

fn unused_name(used: &mut HashSet<String>, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 1;
    while !used.insert(name.to_lowercase()) {
        name = format!("{base} ({n})");
        n += 1;
    }
    name
}

fn distinct<'a>(labels: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    labels
        .map(str::trim)
        .filter(|l| !l.is_empty() && seen.insert(l.to_lowercase()))
        .map(str::to_string)
        .collect()
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn most_common<'a>(labels: impl Iterator<Item = &'a str>) -> Option<(String, usize)> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for label in labels.map(str::trim).filter(|l| !l.is_empty()) {
        let key = |l: &str| {
            l.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        };
        match counts.iter_mut().find(|(seen, _)| key(seen) == key(label)) {
            Some((_, n)) => *n += 1,
            None => counts.push((label, 1)),
        }
    }
    // `max_by_key` keeps the last of equals, so look from the end.
    counts
        .into_iter()
        .rev()
        .max_by_key(|&(_, n)| n)
        .map(|(label, n)| (label.to_string(), n))
}

fn capitalized(label: &str) -> String {
    let mut chars = label.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
