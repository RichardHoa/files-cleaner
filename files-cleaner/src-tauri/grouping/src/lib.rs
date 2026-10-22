//! Suggested Folders from what's inside the loose files of a Scan Root
//! (ADR-0004). A small LLM writes a card per file, an embedding model embeds
//! the cards, and the cards are clustered; each group is named after its
//! most common topic and anchored to a matching existing subfolder. Then
//! the LLM is asked where each read file left out of every group belongs,
//! if anywhere, among the groups and subfolders nearest it.
//! Archives, images and installers are never read. PDFs with no text
//! (scans) are read by OCR, and the ones it can't read, or all of them
//! with no OCR engine, are a kind too: each kind is one group, unless the
//! file's name ties it to a group of read files.
//!
//! Everything that needs llama.cpp or PDFium lives in this crate, so `core`
//! stays pure Rust. `analyse` is the entry point; `Analysis::regroup` groups
//! again with new settings and runs no model.

mod card;
mod cluster;
mod kinds;
mod llama;
mod marks;
mod ocr;
mod place;
mod read;
mod store;
mod trace;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use files_cleaner_core::{ScanEntry, ScanResult};
use serde::{Deserialize, Serialize};

pub use card::Card;
pub use cluster::Vector;
pub use llama::LlamaModels;
pub use place::{Choice, Question};
pub use read::{pdf_child_main, Content};
pub use store::CardStore;
pub use trace::{clear_trace, trace_to};

use cluster::{centroid, cosine, normalized};
use place::Answer;
use store::{content_hash, ModelIds, Stored};

/// The two Grouping Models, behind an interface so the engine can be driven
/// with fakes.
pub trait Models {
    /// Identifies the model that writes cards: stored cards made by
    /// another are not used.
    fn card_model_id(&self) -> String;
    /// Identifies the embedding model, likewise.
    fn embedding_model_id(&self) -> String;
    /// The card for a file. An error leaves the file with an empty card, so
    /// it is grouped by its name alone.
    fn card(&mut self, content: &Content) -> Result<Card, String>;
    /// The folder a file left out of every Suggested Folder belongs in, by
    /// its place in `question.folders`, or `None`. An error leaves it out.
    fn place(&mut self, question: &Question) -> Result<Option<usize>, String>;
    /// One vector per text, in order.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String>;
    /// Where the models ran, such as "Metal GPU" or "CPU"; `None` if they
    /// haven't run.
    fn device(&self) -> Option<String>;
}

/// How the files are grouped.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// How closely files must match to share a folder, 0–1.
    pub folder_threshold: f32,
    /// How closely a group must match an existing subfolder to go into it, 0–1.
    pub existing_folder_threshold: f32,
    /// The fewest files a Suggested Folder holds.
    pub smallest_folder: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            folder_threshold: 0.86,
            existing_folder_threshold: 0.86,
            smallest_folder: 2,
        }
    }
}

/// What `analyse` works from.
pub struct Input<'a> {
    /// A fresh scan of the Scan Root: its loose files are grouped, and its
    /// subfolders are where groups may go.
    pub scan: &'a ScanResult,
    pub settings: Settings,
    pub store: &'a CardStore,
    /// The PDFium library bundled with the app; without it, PDFs are read
    /// by name.
    pub pdfium: Option<&'a Path>,
}

/// How far an analysis has got.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Files read, or placed, so far.
    pub finished: usize,
    pub total: usize,
    /// The file being read or placed.
    pub current: PathBuf,
    /// Whether the files are read, or the files left out are being placed.
    pub placing: bool,
}

/// The longest an analysis waits between progress reports.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Why an analysis ended without a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stopped {
    Cancelled,
    /// The embedding model failed, so nothing can be compared.
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

/// The Suggested Folders for a Scan Root's loose files.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grouping {
    /// Largest first.
    pub folders: Vec<SuggestedFolder>,
    /// Files in no Suggested Folder, which stay where they are.
    pub ungrouped: Vec<PathBuf>,
    /// The kind of each file, grouped or not, such as "Documents".
    pub kinds: BTreeMap<PathBuf, &'static str>,
    /// Each read file's one-sentence summary, where it has one.
    pub summaries: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedFolder {
    pub name: String,
    pub files: Vec<PathBuf>,
    /// The existing subfolder of the Scan Root it matches, if any.
    pub existing: Option<PathBuf>,
    /// Whether it gathers a kind that is never read: archives, images,
    /// installers or scanned PDFs.
    pub fixed_kind: bool,
}

/// Everything the models said about a Scan Root, enough to group again
/// with other settings.
pub struct Analysis {
    pub grouping: Grouping,
    /// Where the models ran; `None` when every file's card was stored.
    pub device: Option<String>,
    /// The loose files that were read, with their cards and vectors.
    files: Vec<PathBuf>,
    cards: Vec<Card>,
    vectors: Vec<Vector>,
    /// The loose files of each kind that is never read, scanned PDFs among them.
    fixed: BTreeMap<&'static str, Vec<PathBuf>>,
    subfolders: Vec<Subfolder>,
    /// What the LLM answered for each placing question, by its prompt: the
    /// folder's label, or none.
    answers: HashMap<String, Option<String>>,
}

/// An existing subfolder of the Scan Root, a place a group may go.
struct Subfolder {
    path: PathBuf,
    name: String,
    /// The centroid of its sampled files' vectors, or of its name's.
    centroid: Vector,
    /// Its sampled files' topics, each once.
    topics: Vec<String>,
    /// Its sampled files' names.
    files: Vec<String>,
}

/// Where a file the LLM places goes.
#[derive(Clone, Copy)]
enum Target {
    /// A Suggested Folder, by its place in the folders.
    Folder(usize),
    Subfolder(usize),
}

/// A placing question the LLM hasn't answered: the file, by its place in
/// the read files, the prompt and the question.
type Unanswered = (usize, String, Question);

/// Files sampled from inside each existing subfolder to describe it.
const FOLDER_SAMPLE: usize = 5;
/// Texts embedded between checks for cancel.
const EMBED_CHUNK: usize = 16;

/// Reads the loose files of `input.scan`, has `models` card and embed them
/// (or takes their stored cards), groups them by `input.settings`, and has
/// `models` place the files left out (or takes the stored answers). Cards
/// and answers are stored as they're made, so a cancelled analysis keeps them.
/// `cancel` is checked between files; `on_progress` is called at most every
/// `PROGRESS_INTERVAL`, and once at the end of reading.
pub fn analyse(
    input: &Input,
    models: &mut dyn Models,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(Progress),
) -> Result<Analysis, Stopped> {
    let scan = input.scan;
    let mut fixed: BTreeMap<&'static str, Vec<PathBuf>> = BTreeMap::new();
    let mut to_read: Vec<&ScanEntry> = Vec::new();
    let mut inside: BTreeMap<&str, Vec<&ScanEntry>> = BTreeMap::new();
    for entry in &scan.entries {
        if entry.is_package {
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

    let (card_id, embedding_id) = (models.card_model_id(), models.embedding_model_id());
    let ids = ModelIds {
        card: &card_id,
        embedding: &embedding_id,
    };
    let mut all: Vec<&ScanEntry> = to_read
        .iter()
        .copied()
        .chain(samples.iter().flat_map(|(_, s)| s.iter().copied()))
        .collect();
    let mut stored: Vec<(Option<blake3::Hash>, Stored)> = Vec::with_capacity(all.len());
    let mut last_report: Option<Instant> = None;
    for (finished, entry) in all.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stopped::Cancelled);
        }
        if last_report.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL) {
            on_progress(Progress {
                finished,
                total: all.len(),
                current: entry.path.clone(),
                placing: false,
            });
            last_report = Some(Instant::now());
        }
        let started = Instant::now();
        let hash = content_hash(&entry.path);
        let name = &entry.name;
        let entry = match hash.as_ref().and_then(|h| input.store.get(&ids, h)) {
            Some(found) => {
                trace::time(&format!(
                    "{name}: saved card, {}\n",
                    trace::secs(started.elapsed())
                ));
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
                found
            }
            None => {
                trace::time(&format!("{name}\n"));
                let content = read::read(&entry.path, &entry.name, input.pdfium);
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
                made
            }
        };
        stored.push((hash, entry));
    }
    if let Some(last) = all.last() {
        on_progress(Progress {
            finished: all.len(),
            total: all.len(),
            current: last.path.clone(),
            placing: false,
        });
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(Stopped::Cancelled);
    }

    // Loose scanned PDFs are a kind of their own, grouped like images; one
    // sampled from a subfolder still stands for it by its name.
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

    // Every file is carded before any is embedded, then the cards missing a
    // vector and the names of subfolders with nothing to sample are embedded.
    let missing: Vec<usize> = (0..stored.len())
        .filter(|&i| stored[i].1.embedding.is_none())
        .collect();
    let unsampled: Vec<&str> = samples
        .iter()
        .filter(|(_, s)| s.is_empty())
        .map(|(f, _)| *f)
        .collect();
    let texts: Vec<String> = missing
        .iter()
        .map(|&i| stored[i].1.card.text(&read::name_words(&all[i].name)))
        .chain(unsampled.iter().map(|f| f.to_string()))
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
    let name_vectors = vectors.split_off(missing.len());
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
    let subfolders = samples
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
                placing: true,
            });
            last_report = Some(Instant::now());
        }
        let started = Instant::now();
        let name = &question.file_name;
        let answer = match input.store.get_answer(&card_id, prompt) {
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

impl Analysis {
    /// The Suggested Folders by `settings`, from what the models already
    /// said: no model runs. A file left out goes where the LLM placed it
    /// when it was asked the same question.
    pub fn regroup(&self, settings: &Settings) -> Grouping {
        self.grouped(settings).0
    }

    /// The grouping, and the placing questions that have no answer yet.
    fn grouped(&self, settings: &Settings) -> (Grouping, Vec<Unanswered>) {
        let folder_names: Vec<String> = self.subfolders.iter().map(|f| f.name.clone()).collect();
        let mut folders = Vec::new();
        let mut ungrouped = Vec::new();
        let mut used_names: HashSet<String> = HashSet::new();

        let topics: Vec<Option<String>> = self.cards.iter().map(Card::topic_key).collect();
        // The same topic, or topics sharing a mark: "IELTS Speaking" and "IELTS Writing".
        let same_topic = |i: usize, j: usize| match (&topics[i], &topics[j]) {
            (Some(a), Some(b)) => {
                a == b || marks::share_mark(&self.cards[i].topic, &self.cards[j].topic)
            }
            _ => false,
        };
        let clusters = cluster::cluster(
            &self.vectors,
            same_topic,
            settings.folder_threshold,
            settings.smallest_folder,
        );
        let mut named: Vec<(String, &[usize])> = Vec::new();
        // Each Suggested Folder of read files, by its place in `folders`, with its centroid and topics.
        let mut topical: Vec<(usize, Vector, Vec<String>)> = Vec::new();

        // A file that is never read joins the first group with a file name
        // sharing a mark with its own: `Lab5_StudentID.zip` with `Lab5_Report.docx`.
        let group_marks: Vec<HashSet<String>> = clusters
            .groups
            .iter()
            .map(|members| {
                members
                    .iter()
                    .flat_map(|&i| marks::name_marks(&file_name(&self.files[i])))
                    .collect()
            })
            .collect();
        let mut joined: Vec<Vec<PathBuf>> = vec![Vec::new(); clusters.groups.len()];
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
            used_names.insert(kind.to_lowercase());
            let existing = kinds::matching_folder(kind, &folder_names)
                .and_then(|name| self.subfolders.iter().find(|f| f.name == name))
                .map(|f| f.path.clone());
            folders.push(SuggestedFolder {
                name: kind.to_string(),
                files,
                existing,
                fixed_kind: true,
            });
        }

        for (members, joined) in clusters.groups.iter().zip(joined) {
            let centre = centroid(members.iter().map(|&i| &self.vectors[i])).unwrap_or_default();
            let existing = self
                .subfolders
                .iter()
                .map(|f| (f, cosine(&centre, &f.centroid)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .filter(|(_, s)| *s >= settings.existing_folder_threshold)
                .map(|(f, _)| f.path.clone());
            // Topics that differ but share a mark name the folder after it: "IELTS".
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
            let base = (!agreed)
                .then(|| marks::shared_mark(&member_topics))
                .flatten()
                .map(str::to_string)
                .or_else(|| most_common(member_topics.iter().copied()))
                .or_else(|| most_common(members.iter().map(|&i| self.cards[i].kind.as_str())))
                .map(|label| capitalized(&label))
                .unwrap_or_else(|| self.majority_kind(members));
            let name = unused_name(&mut used_names, &base);
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
                fixed_kind: false,
            });
        }

        // Every question is put before any file is placed, so none depends on another's answer.
        let questions: Vec<(usize, String, Question, Vec<Target>)> = clusters
            .ungrouped
            .iter()
            .filter_map(|&i| {
                let (question, targets) = self.question(i, &folders, &topical)?;
                Some((i, place::prompt(&question), question, targets))
            })
            .collect();
        let mut unanswered = Vec::new();
        let mut placed: BTreeMap<usize, String> = BTreeMap::new();
        // The Suggested Folder made for each subfolder a file was placed in.
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
                        fixed_kind: false,
                    });
                    folders.len() - 1
                }),
            };
            folders[folder].files.push(self.files[i].clone());
            placed.insert(i, folders[folder].name.clone());
        }
        ungrouped.extend(
            clusters
                .ungrouped
                .iter()
                .filter(|i| !placed.contains_key(i))
                .map(|&i| self.files[i].clone()),
        );
        if trace::enabled() {
            let similarity = |i: usize, j: usize| {
                cluster::similarity(&self.vectors[i], &self.vectors[j], same_topic(i, j))
            };
            let report =
                self.similarity_report(settings, &named, &clusters.ungrouped, &placed, similarity);
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

    /// The placing question for read file `i`, left out of every folder,
    /// and where each of its choices goes: the Suggested Folders of read
    /// files and the subfolders none of them goes into, most like it first.
    /// `None` when its card is empty or there's nowhere to go.
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

    /// For tuning the thresholds: each Suggested Folder made of read files,
    /// with the files outside it most like it, then each file left out of
    /// every folder with the folder most like it and where the LLM placed it. "Like" is the average similarity to
    /// the folder's files, what clustering compares with the folder threshold.
    fn similarity_report(
        &self,
        settings: &Settings,
        folders: &[(String, &[usize])],
        ungrouped: &[usize],
        placed: &BTreeMap<usize, String>,
        similarity: impl Fn(usize, usize) -> f32,
    ) -> String {
        /// Outside files listed per folder.
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

    /// The kind most of `members` are, as a folder name.
    fn majority_kind(&self, members: &[usize]) -> String {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for &i in members {
            *counts
                .entry(kinds::kind(&extension(&self.files[i])))
                .or_insert(0) += 1;
        }
        counts
            .into_iter()
            .max_by_key(|&(_, c)| c)
            .map_or("Files", |(k, _)| k)
            .to_string()
    }
}

/// `base`, with a Clash Suffix if it's in `used` (ignoring case), as for
/// names taken on disk; the name is then used.
fn unused_name(used: &mut HashSet<String>, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 1;
    while !used.insert(name.to_lowercase()) {
        name = format!("{base} ({n})");
        n += 1;
    }
    name
}

/// The non-empty labels, trimmed, each once (ignoring case), as first written.
fn distinct<'a>(labels: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    labels
        .map(str::trim)
        .filter(|l| !l.is_empty() && seen.insert(l.to_lowercase()))
        .map(str::to_string)
        .collect()
}

/// The extension of `path`, lowercased; empty if none.
fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// The file name of `path`, or nothing.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The non-empty label most files carry (ignoring case), as first written;
/// the first of equals wins.
fn most_common<'a>(labels: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for label in labels.map(str::trim).filter(|l| !l.is_empty()) {
        match counts
            .iter_mut()
            .find(|(seen, _)| seen.to_lowercase() == label.to_lowercase())
        {
            Some((_, n)) => *n += 1,
            None => counts.push((label, 1)),
        }
    }
    // `max_by_key` keeps the last of equals, so look from the end.
    counts
        .into_iter()
        .rev()
        .max_by_key(|&(_, n)| n)
        .map(|(label, _)| label.to_string())
}

fn capitalized(label: &str) -> String {
    let mut chars = label.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
