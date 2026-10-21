//! Throwaway, read-only prototype: do Suggested Folders made from what's
//! inside the loose files of a Scan Root beat grouping by kind, name and
//! date? See README.md.

mod baseline;
mod cluster;
mod embed;
mod extract;
mod keywords;
mod report;
mod scan;

use clap::Parser;
use extract::{Content, Limits};
use report::{Matched, Report, SuggestedFolder};
use std::collections::HashSet;
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
    /// Content grouper: lowest average cosine similarity at which two groups merge (0–1).
    #[arg(long, default_value_t = 0.5)]
    threshold: f32,
    /// Both groupers: groups with fewer files count as Ungrouped.
    #[arg(long, default_value_t = 3)]
    min_size: usize,
    /// Content grouper: lowest similarity to an existing subfolder's centroid to suggest it.
    #[arg(long, default_value_t = 0.45)]
    anchor_threshold: f32,
    /// Content grouper: most files read from each existing subfolder (recursively) for its centroid.
    #[arg(long, default_value_t = 40)]
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
    /// Skip reading PDFs larger than this many MB (they are grouped by name only).
    #[arg(long, default_value_t = 100)]
    max_pdf_mb: u64,
    /// A folder with `model.onnx` and `tokenizer.json` of any sentence-transformers ONNX export.
    /// Default: all-MiniLM-L6-v2, downloaded once into the user cache folder.
    #[arg(long)]
    model_dir: Option<PathBuf>,
}

fn main() -> ExitCode {
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
    let (model_dir, model_label) = match &args.model_dir {
        Some(dir) => {
            let dir = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;
            let label = format!("{} (from --model-dir)", dir.file_name().unwrap_or_default().to_string_lossy());
            (dir, label)
        }
        None => {
            let dir = embed::default_model_dir().ok_or("no user cache folder; pass --model-dir")?;
            warn_if_inside(&root, &dir, "model cache");
            stages.time("download model (first run only)", || embed::ensure_default_model(&dir))?;
            (dir, format!("{} (sentence-transformers/all-MiniLM-L6-v2, ONNX, 384 dimensions)", embed::DEFAULT_MODEL))
        }
    };
    let limits = Limits {
        max_chars: args.max_chars,
        pdf_timeout: Duration::from_secs(args.pdf_timeout),
        max_pdf_bytes: args.max_pdf_mb * 1024 * 1024,
    };

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
    let mut embedder = stages.time("load model", || embed::Embedder::load(&model_dir, model_label))?;
    let texts: Vec<String> = contents.iter().map(|c| c.text.clone()).collect();
    let vectors = stages.time("embed loose files", || embedder.embed(&texts))?;
    let folders = stages.time("read and embed subfolders", || -> Result<Vec<report::Folder>, String> {
        let mut folders = Vec::new();
        for (path, name) in listing.subfolders.iter().zip(&folder_names) {
            let sample = scan::sample_files(path, args.folder_sample);
            let mut texts: Vec<String> =
                sample.iter().map(|i| extract::read(i, &limits).text).collect();
            let sampled = texts.len();
            // An empty folder is still a destination; its name is all we know.
            if texts.is_empty() {
                texts.push(extract::name_words(name));
            }
            if let Some(centroid) = embed::centroid(&embedder.embed(&texts)?) {
                folders.push(report::Folder { name: name.clone(), centroid, sampled });
            }
        }
        Ok(folders)
    })?;
    let clusters = stages.time("cluster", || cluster::agglomerate(&vectors, args.threshold, args.min_size));
    let content = stages.time("anchor and name", || {
        let terms: Vec<HashSet<String>> = contents.iter().map(|c| keywords::terms(&c.text)).collect();
        let corpus = keywords::Corpus::new(&terms);
        let mut used_names: HashSet<String> = HashSet::new();
        clusters
            .groups
            .iter()
            .map(|members| {
                let centroid = embed::centroid(members.iter().map(|&i| &vectors[i])).unwrap_or_default();
                let matched = folders
                    .iter()
                    .map(|f| (f, embed::cosine(&centroid, &f.centroid)))
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .filter(|(_, s)| *s >= args.anchor_threshold)
                    .map(|(f, s)| Matched::BySimilarity(f.name.clone(), s));
                let group_terms: Vec<&HashSet<String>> = members.iter().map(|&i| &terms[i]).collect();
                let top = corpus.top_terms(&group_terms, 3, 5);
                let base = keywords::folder_name(&top).unwrap_or_else(|| majority_kind(items, members));
                let mut name = base.clone();
                let mut n = 2;
                while !used_names.insert(name.to_lowercase()) {
                    name = format!("{base} ({n})");
                    n += 1;
                }
                let phrases = top.iter().map(|(t, c)| format!("{t} ({c}/{} files)", members.len())).collect();
                SuggestedFolder { name, matched, phrases, members: members.clone() }
            })
            .collect::<Vec<_>>()
    });

    let report = Report {
        root: &root,
        items,
        contents: &contents,
        skipped: listing.skipped.iter().map(|(_, s)| *s).collect(),
        model: &embedder.model_label,
        settings: format!(
            "--threshold {} --min-size {} --anchor-threshold {} --folder-sample {} --max-chars {} --split-at {}",
            args.threshold, args.min_size, args.anchor_threshold, args.folder_sample, args.max_chars, args.split_at
        ),
        folders: &folders,
        baseline: &baseline,
        baseline_ungrouped: &baseline_ungrouped,
        content: &content,
        content_ungrouped: &clusters.ungrouped,
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

fn majority_kind(items: &[scan::Item], members: &[usize]) -> String {
    let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
    for &i in members {
        *counts.entry(baseline::kind(&items[i].ext)).or_insert(0) += 1;
    }
    counts.into_iter().max_by_key(|&(_, c)| c).map_or("Files", |(k, _)| k).to_string()
}
