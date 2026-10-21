//! Naming a group from its text with no LLM: TF-IDF over words and word
//! pairs, where a term scores by how many of the group's files contain it,
//! weighted by how rare it is across the whole Scan Root.

use std::collections::{HashMap, HashSet};

const STOPWORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "also", "am", "an", "and", "any", "are", "as", "at",
    "be", "because", "been", "before", "being", "below", "between", "both", "but", "by", "can", "could", "did", "do",
    "does", "doing", "down", "during", "each", "etc", "few", "for", "from", "further", "had", "has", "have", "having",
    "he", "her", "here", "hers", "him", "his", "how", "i", "if", "in", "into", "is", "it", "its", "just", "may", "me",
    "more", "most", "must", "my", "no", "nor", "not", "now", "of", "off", "on", "once", "only", "or", "other", "our",
    "ours", "out", "over", "own", "per", "same", "shall", "she", "should", "so", "some", "such", "than", "that",
    "the", "their", "them", "then", "there", "these", "they", "this", "those", "through", "to", "too", "under",
    "until", "up", "upon", "us", "very", "via", "was", "we", "were", "what", "when", "where", "which", "while", "who",
    "whom", "why", "will", "with", "within", "without", "would", "you", "your", "yours",
    // file-name noise
    "copy", "final", "new", "untitled", "file", "files", "document", "img", "image", "dsc", "pdf", "docx", "doc",
    "pptx", "xlsx", "png", "jpg", "jpeg", "heic", "txt", "www", "http", "https", "com", "org", "html",
];

/// Lowercased words of 3+ letters (letters, or letters with digits such as
/// a course code `cs101`), without stopwords or pure numbers.
pub fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && w.chars().count() <= 24)
        .filter(|w| w.chars().any(char::is_alphabetic))
        .filter(|w| w.chars().filter(|c| c.is_numeric()).count() <= 4)
        .map(str::to_lowercase)
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// A file name's words without its extension (every `.dmg` would share
/// `dmg`), split on separators only, so `CS101` stays one word.
pub fn name_words(name: &str) -> HashSet<String> {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    tokens(stem).into_iter().collect()
}

/// Each file's distinct terms: its words, and pairs of adjacent words within
/// one clause (pairs never span punctuation such as `.` or `:`).
pub fn terms(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for clause in text.split(['.', ',', ':', ';', '!', '?', '(', ')', '\n', '|', '•']) {
        let words = tokens(clause);
        set.extend(words.windows(2).filter(|p| p[0] != p[1]).map(|p| format!("{} {}", p[0], p[1])));
        set.extend(words);
    }
    set
}

pub struct Corpus {
    files: usize,
    document_frequency: HashMap<String, usize>,
}

impl Corpus {
    pub fn new(all: &[HashSet<String>]) -> Self {
        let mut document_frequency = HashMap::new();
        for set in all {
            for t in set {
                *document_frequency.entry(t.clone()).or_insert(0) += 1;
            }
        }
        Self { files: all.len(), document_frequency }
    }

    fn idf(&self, term: &str) -> f32 {
        let df = self.document_frequency.get(term).copied().unwrap_or(1) as f32;
        ((self.files as f32 + 1.0) / df).ln()
    }

    /// The best terms of a Suggested Folder's files, with how many of them
    /// contain each. Terms shared by at least two files come first; terms of
    /// a single file only fill up to `at_least`. A word already inside a
    /// better-scoring pair is dropped.
    pub fn top_terms(&self, group: &[&HashSet<String>], at_least: usize, limit: usize) -> Vec<(String, usize)> {
        let mut coverage: HashMap<&str, usize> = HashMap::new();
        for set in group {
            for t in set.iter() {
                *coverage.entry(t.as_str()).or_insert(0) += 1;
            }
        }
        let shared = group.len().min(2);
        let mut scored: Vec<(&str, usize, f32)> = coverage
            .into_iter()
            .map(|(t, c)| {
                let pair_bonus = if t.contains(' ') { 1.15 } else { 1.0 };
                (t, c, c as f32 / group.len() as f32 * self.idf(t) * pair_bonus)
            })
            .collect();
        scored.sort_by(|a, b| (b.1 >= shared).cmp(&(a.1 >= shared)).then(b.2.total_cmp(&a.2)).then(a.0.cmp(b.0)));
        let mut chosen: Vec<(String, usize)> = Vec::new();
        for (t, c, _) in scored {
            if c < shared && chosen.len() >= at_least {
                break;
            }
            let overlaps = chosen.iter().any(|(p, _)| {
                p.split(' ').any(|w| w == t) || t.split(' ').any(|w| w == p)
            });
            if !overlaps {
                chosen.push((t.to_string(), c));
            }
            if chosen.len() == limit {
                break;
            }
        }
        chosen
    }
}

/// A folder name from the top terms: the first one, plus the second when the
/// first is a single word. Title-cased; course codes and acronyms uppercase.
pub fn folder_name(top: &[(String, usize)]) -> Option<String> {
    let first = &top.first()?.0;
    let mut words: Vec<&str> = first.split(' ').collect();
    if words.len() == 1 {
        if let Some((second, _)) = top.get(1) {
            words.extend(second.split(' '));
        }
    }
    Some(words.iter().map(|w| title_case(w)).collect::<Vec<_>>().join(" "))
}

pub fn title_case(word: &str) -> String {
    if word.chars().any(|c| c.is_numeric()) {
        return word.to_uppercase();
    }
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}
