//! The Markdown report the developer reads to judge the two groupers.

use crate::embed::Vector;
use crate::extract::{Content, Method};
use crate::scan::{Item, Skip};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;
use std::time::Duration;

pub enum Matched {
    /// Baseline: the subfolder's name contains the Suggested Folder's token or kind.
    ByName(String),
    /// Content: cosine similarity between the Suggested Folder's and the subfolder's centroid.
    BySimilarity(String, f32),
}

pub struct SuggestedFolder {
    pub name: String,
    pub matched: Option<Matched>,
    pub phrases: Vec<String>,
    pub members: Vec<usize>,
}

pub struct Folder {
    pub name: String,
    pub centroid: Vector,
    /// Files read for the centroid; 0 means only the folder's name was used.
    pub sampled: usize,
}

pub struct Report<'a> {
    pub root: &'a Path,
    pub items: &'a [Item],
    pub contents: &'a [Content],
    pub skipped: Vec<Skip>,
    pub model: &'a str,
    pub settings: String,
    pub folders: &'a [Folder],
    pub baseline: &'a [SuggestedFolder],
    pub baseline_ungrouped: &'a [usize],
    pub content: &'a [SuggestedFolder],
    pub content_ungrouped: &'a [usize],
}

impl Report<'_> {
    pub fn render(&self, stages: &[(&str, Duration)], total: Duration) -> String {
        let mut md = String::new();
        let _ = self.write(&mut md, stages, total);
        md
    }

    fn write(&self, md: &mut String, stages: &[(&str, Duration)], total: Duration) -> std::fmt::Result {
        writeln!(md, "# Suggested Folders prototype report\n")?;
        writeln!(md, "- **Scan Root:** `{}`", self.root.display())?;
        writeln!(md, "- **Loose files:** {}", self.items.len())?;
        let mut by_method: BTreeMap<Method, usize> = BTreeMap::new();
        for c in self.contents {
            *by_method.entry(c.method).or_insert(0) += 1;
        }
        let methods: Vec<String> = [Method::PdfText, Method::OfficeXml, Method::PlainText, Method::NameOnly]
            .iter()
            .map(|m| format!("{} {}", by_method.get(m).copied().unwrap_or(0), m.label()))
            .collect();
        let readable = self.contents.iter().filter(|c| c.method != Method::NameOnly).count();
        writeln!(md, "- **Readable content:** {readable} of {} ({})", self.items.len(), methods.join(", "))?;
        if !self.skipped.is_empty() {
            let mut counts: BTreeMap<Skip, usize> = BTreeMap::new();
            for s in &self.skipped {
                *counts.entry(*s).or_insert(0) += 1;
            }
            let parts: Vec<String> = counts.iter().map(|(s, n)| format!("{n} {}", s.label())).collect();
            writeln!(md, "- **Left out:** {}", parts.join(", "))?;
        }
        let folders: Vec<String> = self
            .folders
            .iter()
            .map(|f| match f.sampled {
                0 => format!("`{}/` (empty: name only)", f.name),
                n => format!("`{}/` ({n} files read)", f.name),
            })
            .collect();
        writeln!(
            md,
            "- **Existing subfolders:** {}",
            if folders.is_empty() { "none".to_string() } else { folders.join(", ") }
        )?;
        writeln!(md, "- **Embedding model:** {}", self.model)?;
        writeln!(md, "- **Settings:** `{}`", self.settings)?;
        writeln!(md, "- **Total time:** {}\n", secs(total))?;
        writeln!(md, "| Stage | Time |\n|---|---|")?;
        for (name, d) in stages {
            writeln!(md, "| {name} | {} |", secs(*d))?;
        }

        writeln!(md, "\n# A. Baseline: kind, name and date (no ML)\n")?;
        self.suggested(md, self.baseline, self.baseline_ungrouped)?;
        writeln!(md, "\n# B. Content: embeddings, clustering, keyword names\n")?;
        self.suggested(md, self.content, self.content_ungrouped)
    }

    fn suggested(&self, md: &mut String, folders: &[SuggestedFolder], ungrouped: &[usize]) -> std::fmt::Result {
        let grouped: usize = folders.iter().map(|g| g.members.len()).sum();
        writeln!(md, "{} Suggested Folders holding {grouped} files; {} ungrouped.", folders.len(), ungrouped.len())?;
        for g in folders {
            writeln!(md, "\n## {} ({} files)\n", g.name, g.members.len())?;
            let goes_to = match &g.matched {
                Some(Matched::ByName(f)) => format!("existing `{f}/` (name match)"),
                Some(Matched::BySimilarity(f, s)) => format!("existing `{f}/` (similarity {s:.2})"),
                None => "new folder".to_string(),
            };
            writeln!(md, "- **Goes to:** {goes_to}")?;
            if !g.phrases.is_empty() {
                writeln!(md, "- **Why:** {}", g.phrases.join("; "))?;
            }
            writeln!(md)?;
            self.files(md, &g.members)?;
        }
        writeln!(md, "\n## Ungrouped ({} files)\n", ungrouped.len())?;
        self.files(md, ungrouped)
    }

    fn files(&self, md: &mut String, members: &[usize]) -> std::fmt::Result {
        for &i in members {
            writeln!(md, "- `{}` · {}", self.items[i].name, self.contents[i].method.label())?;
        }
        Ok(())
    }
}

fn secs(d: Duration) -> String {
    format!("{:.2} s", d.as_secs_f64())
}
