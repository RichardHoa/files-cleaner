//! Throwaway, read-only prototype: do Suggested Folders made from what's
//! inside the loose files of a Scan Root beat grouping by kind, name and
//! date? See README.md.

mod baseline;
mod cluster;
mod download;
mod embed;
mod extract;
mod keywords;
mod llm;
mod report;
mod scan;

use clap::Parser;
use extract::{Content, Limits};
use llm::Card;
use report::{Matched, Report, SuggestedFolder};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(about = "Suggest folders for the loose files of a Scan Root, by kind/name/date and by content. Read-only.")]
struct Args {
    /// The folder to analyse, e.g. ~/Downloads. Nothing in it is changed.
    scan_root: PathBuf,
    /// Where to write the Markdown report; not a symlink.
    /// Default: `reports/grouping-report.md` in this prototype's folder.
    #[arg(long)]
    report: Option<PathBuf>,
    /// Content grouper: lowest average similarity of the cards at which two groups merge (0–1).
    #[arg(long, default_value_t = 0.86)]
    threshold: f32,
    /// Both groupers: groups with fewer files count as Ungrouped.
    #[arg(long, default_value_t = 2)]
    min_size: usize,
    /// Content grouper: lowest similarity to an existing subfolder's centroid to suggest it.
    #[arg(long, default_value_t = 0.86)]
    anchor_threshold: f32,
    /// Content grouper: most files read (and carded by the LLM) from each existing subfolder, recursively.
    #[arg(long, default_value_t = 5)]
    folder_sample: usize,
    /// Most characters of content read from each file.
    #[arg(long, default_value_t = 2000)]
    max_chars: usize,
    /// Baseline grouper: a kind with at least this many files is split by name token, then by month.
    #[arg(long, default_value_t = 8)]
    split_at: usize,
    /// Give up on a PDF's text after this many seconds.
    #[arg(long, default_value_t = 10)]
    pdf_timeout: u64,
    /// A PDFium library file (`libpdfium.dylib`, `pdfium.dll`, `libpdfium.so`) to use.
    /// Default: PDFium from bblanchon/pdfium-binaries, downloaded once into the user cache folder.
    #[arg(long)]
    pdfium: Option<PathBuf>,
    /// A folder with `model.onnx` (plus `model.onnx_data` if it has one) and `tokenizer.json`
    /// of any sentence-transformers ONNX export. Default: multilingual-e5-small, downloaded once
    /// into the user cache folder.
    #[arg(long)]
    model_dir: Option<PathBuf>,
    /// How the embedding model pools token vectors.
    #[arg(long, value_enum, default_value = "mean")]
    pooling: embed::Pooling,
    /// Text put before every text the model embeds. Default: "query: " for the default model,
    /// none with --model-dir.
    #[arg(long)]
    prefix: Option<String>,
    /// A folder with `model.gguf` (a Qwen2 or Qwen2.5 GGUF) and its `tokenizer.json`.
    /// Default: Qwen2.5-1.5B-Instruct, 4-bit, downloaded once into the user cache folder.
    #[arg(long)]
    llm_dir: Option<PathBuf>,
    /// Run the LLM on the CPU even when a Metal GPU is there.
    #[arg(long)]
    cpu: bool,
}

fn main() -> ExitCode {
    let raw: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if raw.get(1).is_some_and(|a| a == extract::PDF_CHILD_FLAG) {
        let max_chars = raw.get(4).and_then(|n| n.to_str()?.parse().ok()).unwrap_or(2000);
        let (Some(pdfium), Some(path)) = (raw.get(2), raw.get(3)) else { return ExitCode::FAILURE };
        // A panic or a crash in PDFium just ends this child; the parent sees a failure.
        return match extract::pdf_child(Path::new(pdfium), Path::new(path), max_chars) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Canonical form of a path that may not exist yet (its parent must).
fn canonical_target(path: &Path) -> Result<PathBuf, String> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let name = path.file_name().ok_or_else(|| format!("{} has no file name", path.display()))?;
    let parent = parent.canonicalize().map_err(|e| format!("{}: {e}", parent.display()))?;
    Ok(parent.join(name))
}

/// Any folder can be the Scan Root, even one holding the report or the model
/// cache (the repo, the home folder). Writing there never changes the files
/// being grouped: the report is written after the analysis, the cache before
/// it, and neither is ever one of those files. It's still worth saying.
fn warn_if_inside(root: &Path, path: &Path, what: &str) {
    if path.starts_with(root) {
        eprintln!("note: the {what} ({}) is inside the Scan Root; it is the only thing written there", path.display());
    }
}

/// Most similar files listed for each ungrouped file in the report.
const NEAREST_SHOWN: usize = 3;

struct Stages(Vec<(&'static str, Duration)>);

impl Stages {
    fn time<T>(&mut self, name: &'static str, f: impl FnOnce() -> T) -> T {
        eprintln!("{name}…");
        let start = Instant::now();
        let out = f();
        self.0.push((name, start.elapsed()));
        out
    }
}

fn run(args: Args) -> Result<(), String> {
    let started = Instant::now();
    let root = args.scan_root.canonicalize().map_err(|e| format!("{}: {e}", args.scan_root.display()))?;
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    let report_path = match &args.report {
        Some(path) => canonical_target(path)?,
        None => {
            let reports = Path::new(env!("CARGO_MANIFEST_DIR")).join("reports");
            std::fs::create_dir_all(&reports).map_err(|e| format!("{}: {e}", reports.display()))?;
            canonical_target(&reports.join("grouping-report.md"))?
        }
    };
    warn_if_inside(&root, &report_path, "report");
    // Writing follows a symlink, which could point at any file at all.
    if std::fs::symlink_metadata(&report_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("the report path {} is a symlink; pick a plain file path", report_path.display()));
    }
    let mut stages = Stages(Vec::new());
    let (model_dir, model_label, prefix) = match &args.model_dir {
        Some(dir) => {
            let dir = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;
            let label = format!("{} (from --model-dir)", dir.file_name().unwrap_or_default().to_string_lossy());
            (dir, label, args.prefix.clone().unwrap_or_default())
        }
        None => {
            let dir = embed::default_model_dir().ok_or("no user cache folder; pass --model-dir")?;
            warn_if_inside(&root, &dir, "model cache");
            stages.time("download embedding model (first run only)", || embed::ensure_default_model(&dir))?;
            let label = format!("{} (ONNX, 8-bit, 384 dimensions, multilingual)", embed::DEFAULT_MODEL);
            (dir, label, args.prefix.clone().unwrap_or_else(|| embed::DEFAULT_PREFIX.to_string()))
        }
    };
    let (llm_dir, llm_label) = match &args.llm_dir {
        Some(dir) => {
            let dir = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;
            let label = format!("{} (from --llm-dir)", dir.file_name().unwrap_or_default().to_string_lossy());
            (dir, label)
        }
        None => {
            let dir = llm::default_llm_dir().ok_or("no user cache folder; pass --llm-dir")?;
            warn_if_inside(&root, &dir, "LLM cache");
            stages.time("download LLM (first run only)", || llm::ensure_default_llm(&dir))?;
            (dir, format!("{} (GGUF, 4-bit)", llm::DEFAULT_LLM))
        }
    };
    let pdfium = match &args.pdfium {
        Some(lib) => lib.canonicalize().map_err(|e| format!("{}: {e}", lib.display()))?,
        None => stages.time("download PDFium (first run only)", extract::ensure_pdfium)?,
    };
    let limits = Limits { max_chars: args.max_chars, pdf_timeout: Duration::from_secs(args.pdf_timeout), pdfium };

    let listing = stages.time("list loose items", || scan::loose_items(&root)).map_err(|e| e.to_string())?;
    let items = &listing.loose;
    let contents: Vec<Content> = stages.time("read content", || {
        items.iter().map(|i| extract::read(i, &limits)).collect()
    });
    let folder_names: Vec<String> = listing
        .subfolders
        .iter()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned())
        .collect();

    // A. Baseline
    let (baseline, baseline_ungrouped) = stages.time("baseline grouping", || {
        let suggested = baseline::suggest(items, args.min_size, args.split_at);
        let folders: Vec<SuggestedFolder> = suggested
            .folders
            .into_iter()
            .map(|g| SuggestedFolder {
                matched: baseline::match_by_name(&g, &folder_names).map(|f| Matched::ByName(f.to_string())),
                name: g.name,
                phrases: Vec::new(),
                members: g.members,
            })
            .collect();
        (folders, suggested.ungrouped)
    });

    // B. Content
    // Archives, images and installers go into one folder per kind, unread.
    let mut fixed: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut read: Vec<usize> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        match baseline::fixed_kind(&item.ext) {
            Some(kind) => fixed.entry(kind).or_default().push(i),
            None => read.push(i),
        }
    }
    let mut llm = stages.time("load LLM", || llm::Llm::load(&llm_dir, args.cpu))?;
    let mut cards: Vec<Option<Card>> = vec![None; items.len()];
    stages.time("LLM cards", || {
        for (n, &i) in read.iter().enumerate() {
            eprintln!("  card {}/{}: {}", n + 1, read.len(), items[i].name);
            cards[i] = Some(card_or_empty(&mut llm, &contents[i]));
        }
    });
    let mut embedder =
        stages.time("load embedding model", || embed::Embedder::load(&model_dir, args.pooling, prefix, model_label))?;
    let vectors = stages.time("embed cards", || {
        let texts: Vec<String> = read.iter().map(|&i| card_text(cards[i].as_ref(), &contents[i])).collect();
        embedder.embed(&texts)
    })?;
    let folders = stages.time("card and embed subfolders", || -> Result<Vec<report::Folder>, String> {
        let mut folders = Vec::new();
        for (path, name) in listing.subfolders.iter().zip(&folder_names) {
            let sample: Vec<scan::Item> = scan::sample_files(path, args.folder_sample)
                .into_iter()
                .filter(|i| baseline::fixed_kind(&i.ext).is_none())
                .collect();
            // An empty folder is still a destination; its name is all we know.
            let texts: Vec<String> = if sample.is_empty() {
                vec![name.clone()]
            } else {
                sample
                    .iter()
                    .map(|i| {
                        let content = extract::read(i, &limits);
                        card_text(Some(&card_or_empty(&mut llm, &content)), &content)
                    })
                    .collect()
            };
            if let Some(centroid) = embed::centroid(&embedder.embed(&texts)?) {
                folders.push(report::Folder { name: name.clone(), centroid, sampled: sample.len() });
            }
        }
        Ok(folders)
    })?;
    let (clusters, nearest) = stages.time("cluster", || {
        let topics: Vec<Option<String>> = read.iter().map(|&i| cards[i].as_ref().and_then(Card::topic_key)).collect();
        let sims = cluster::similarities(&vectors, &topics);
        // Back from positions in `read` to positions in `items`.
        let mut nearest = vec![Vec::new(); items.len()];
        for (k, near) in sims.nearest(NEAREST_SHOWN).into_iter().enumerate() {
            nearest[read[k]] = near.into_iter().map(|(j, s)| (read[j], s)).collect();
        }
        let mut clusters = cluster::agglomerate(sims, args.threshold, args.min_size);
        clusters.groups.iter_mut().flatten().chain(&mut clusters.ungrouped).for_each(|k| *k = read[*k]);
        (clusters, nearest)
    });
    let (content, content_ungrouped) = stages.time("anchor and name", || {
        let mut used_names: HashSet<String> = HashSet::new();
        let mut suggested = Vec::new();
        let mut ungrouped = clusters.ungrouped.clone();
        for (kind, members) in fixed {
            if members.len() < args.min_size {
                ungrouped.extend(members);
                continue;
            }
            used_names.insert(kind.to_lowercase());
            let folder = baseline::KindFolder { name: kind.to_string(), match_words: vec![kind.to_lowercase()], members };
            suggested.push(SuggestedFolder {
                matched: baseline::match_by_name(&folder, &folder_names).map(|f| Matched::ByName(f.to_string())),
                name: folder.name,
                phrases: vec![format!("every {} goes here", kind.to_lowercase().trim_end_matches('s'))],
                members: folder.members,
            });
        }
        let position: HashMap<usize, usize> = read.iter().enumerate().map(|(k, &i)| (i, k)).collect();
        for members in &clusters.groups {
            let centroid = embed::centroid(members.iter().map(|i| &vectors[position[i]])).unwrap_or_default();
            let matched = folders
                .iter()
                .map(|f| (f, embed::cosine(&centroid, &f.centroid)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .filter(|(_, s)| *s >= args.anchor_threshold)
                .map(|(f, s)| Matched::BySimilarity(f.name.clone(), s));
            let group_cards: Vec<&Card> = members.iter().filter_map(|&i| cards[i].as_ref()).collect();
            let topics = most_common(group_cards.iter().map(|c| c.topic.as_str()));
            let kinds = most_common(group_cards.iter().map(|c| c.kind.as_str()));
            let base = topics
                .first()
                .or(kinds.first())
                .map(|(t, _)| capitalized(t))
                .unwrap_or_else(|| majority_kind(items, members));
            let mut name = base.clone();
            let mut n = 2;
            while !used_names.insert(name.to_lowercase()) {
                name = format!("{base} ({n})");
                n += 1;
            }
            let count = |label: &str, list: &[(String, usize)]| {
                list.iter().take(3).map(|(t, c)| format!("{label} “{t}” ({c}/{} files)", members.len())).collect::<Vec<_>>()
            };
            let phrases = [count("topic", &topics), count("type", &kinds)].concat();
            suggested.push(SuggestedFolder { name, matched, phrases, members: members.clone() });
        }
        suggested.sort_by(|a, b| b.members.len().cmp(&a.members.len()).then(a.name.cmp(&b.name)));
        ungrouped.sort();
        (suggested, ungrouped)
    });

    let report = Report {
        root: &root,
        items,
        contents: &contents,
        skipped: listing.skipped.iter().map(|(_, s)| *s).collect(),
        model: &format!("{}; LLM: {llm_label} on the {}", embedder.model_label, llm.device_label()),
        cards: &cards,
        settings: format!(
            "--threshold {} --min-size {} --anchor-threshold {} --pooling {} --prefix {:?} --folder-sample {} --max-chars {} --split-at {}",
            args.threshold,
            args.min_size,
            args.anchor_threshold,
            match args.pooling {
                embed::Pooling::Cls => "cls",
                embed::Pooling::Mean => "mean",
            },
            embedder.prefix,
            args.folder_sample,
            args.max_chars,
            args.split_at
        ),
        folders: &folders,
        baseline: &baseline,
        baseline_ungrouped: &baseline_ungrouped,
        content: &content,
        content_ungrouped: &content_ungrouped,
        nearest: &nearest,
        threshold: args.threshold,
    };
    let markdown = report.render(&stages.0, started.elapsed());
    std::fs::write(&report_path, markdown).map_err(|e| format!("{}: {e}", report_path.display()))?;

    println!("Report: {}", report_path.display());
    for (name, d) in &stages.0 {
        println!("  {name:<26} {d:>9.2?}");
    }
    println!("  {:<26} {:>9.2?}", "total", started.elapsed());
    Ok(())
}

/// A failed LLM run (a reply the model never finished, a tensor error) is
/// an empty card: the file is then grouped by its name alone.
fn card_or_empty(llm: &mut llm::Llm, content: &Content) -> Card {
    llm.card(content).unwrap_or_else(|e| {
        eprintln!("  no card for {}: {e}", content.name);
        Card::default()
    })
}

/// What a file is embedded as: its card, or its name when the card is empty.
fn card_text(card: Option<&Card>, content: &Content) -> String {
    card.map(Card::text).filter(|t| !t.is_empty()).unwrap_or_else(|| content.name.clone())
}

/// Non-empty labels by how many files carry them (ignoring case), most
/// first, each shown as first written.
fn most_common<'a>(labels: impl Iterator<Item = &'a str>) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for label in labels.map(str::trim).filter(|l| !l.is_empty()) {
        match counts.iter_mut().find(|(seen, _)| seen.to_lowercase() == label.to_lowercase()) {
            Some((_, n)) => *n += 1,
            None => counts.push((label.to_string(), 1)),
        }
    }
    counts.sort_by_key(|c| std::cmp::Reverse(c.1));
    counts
}

fn capitalized(label: &str) -> String {
    let mut chars = label.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn majority_kind(items: &[scan::Item], members: &[usize]) -> String {
    let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
    for &i in members {
        *counts.entry(baseline::kind(&items[i].ext)).or_insert(0) += 1;
    }
    counts.into_iter().max_by_key(|&(_, c)| c).map_or("Files", |(k, _)| k).to_string()
}
