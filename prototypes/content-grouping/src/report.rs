//! The Markdown report the developer reads to judge the two groupers.

use crate::embed::Vector;
use crate::extract::{Content, Method};
use crate::llm::Card;
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
    /// Per loose file: what the LLM said about it; `None` for files it
    /// didn't read (archives, images, installers).
    pub cards: &'a [Option<Card>],
    pub settings: String,
    pub folders: &'a [Folder],
    pub baseline: &'a [SuggestedFolder],
    pub baseline_ungrouped: &'a [usize],
    pub content: &'a [SuggestedFolder],
    pub content_ungrouped: &'a [usize],
    /// Per loose file: its most similar other files, most similar first.
    pub nearest: &'a [Vec<(usize, f32)>],
    pub threshold: f32,
}

/// Characters of each ungrouped file's content shown, to spot garbled text.
const PREVIEW_CHARS: usize = 120;

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
        writeln!(md, "- **Models:** {}", self.model)?;
        writeln!(md, "- **Settings:** `{}`", self.settings)?;
        writeln!(md, "- **Total time:** {}\n", secs(total))?;
        writeln!(md, "| Stage | Time |\n|---|---|")?;
        for (name, d) in stages {
            writeln!(md, "| {name} | {} |", secs(*d))?;
        }

        writeln!(md, "\n# A. Baseline: kind, name and date (no ML)\n")?;
        self.suggested(md, self.baseline, self.baseline_ungrouped, false)?;
        writeln!(md, "\n# B. Content: archives, images and installers by kind; the rest by LLM cards\n")?;
        self.suggested(md, self.content, self.content_ungrouped, true)?;
        self.closest(md)
    }

    /// Why each file the content grouper left out stayed out: what it read
    /// and how close the nearest files came to the threshold.
    fn closest(&self, md: &mut String) -> std::fmt::Result {
        if self.content_ungrouped.is_empty() {
            return Ok(());
        }
        writeln!(md, "\n## Ungrouped: nearest files\n")?;
        writeln!(
            md,
            "Similarity of the cards (1 for the same topic) to the most similar files; groups merge at {}. \
             A file can be close to one file and still stay out when it is far from the rest of that file's group.\n",
            self.threshold
        )?;
        for &i in self.content_ungrouped {
            let content = &self.contents[i];
            write!(md, "- `{}` · {}", self.items[i].name, content.method.label())?;
            if !content.body.is_empty() {
                let flat = content.body.split_whitespace().collect::<Vec<_>>().join(" ");
                let preview: String = flat.chars().take(PREVIEW_CHARS).collect();
                let more = if flat.chars().count() > PREVIEW_CHARS { "…" } else { "" };
                write!(md, " · “{}{more}”", preview.replace('`', "'"))?;
            }
            write!(md, "{}", self.card(i))?;
            let near: Vec<String> =
                self.nearest[i].iter().map(|&(j, s)| format!("`{}` {s:.2}", self.items[j].name)).collect();
            writeln!(md, "\n  - nearest: {}", if near.is_empty() { "none".to_string() } else { near.join(", ") })?;
        }
        Ok(())
    }

    fn suggested(
        &self,
        md: &mut String,
        folders: &[SuggestedFolder],
        ungrouped: &[usize],
        with_cards: bool,
    ) -> std::fmt::Result {
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
            self.files(md, &g.members, with_cards)?;
        }
        writeln!(md, "\n## Ungrouped ({} files)\n", ungrouped.len())?;
        self.files(md, ungrouped, with_cards)
    }

    fn files(&self, md: &mut String, members: &[usize], with_cards: bool) -> std::fmt::Result {
        for &i in members {
            let card = if with_cards { self.card(i) } else { String::new() };
            writeln!(md, "- `{}` · {}{card}", self.items[i].name, self.contents[i].method.label())?;
        }
        Ok(())
    }

    /// The file's card as ` · topic · type · summary`, or nothing when the
    /// LLM didn't read it.
    fn card(&self, i: usize) -> String {
        match &self.cards[i] {
            None => String::new(),
            Some(c) if c.text().is_empty() => " · (empty card)".to_string(),
            Some(c) => {
                let topic = if c.topic.is_empty() { "no topic" } else { &c.topic };
                format!(" · **{topic}** · {} · {}", c.kind, c.summary).replace('`', "'")
            }
        }
    }
}

fn secs(d: Duration) -> String {
    format!("{:.2} s", d.as_secs_f64())
}
