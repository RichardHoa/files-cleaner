//! The LLM names topics too narrowly, and archives and images are never read: a
//! shared mark ("IELTS", "Lab5") is what ties those files to their subject.

/// Acronyms too common to pick out a subject.
const GENERIC: &[&str] = &[
    "PDF", "HTML", "DOC", "DOCX", "CSV", "API", "FAQ", "USA", "PPT", "JSON", "XML",
];

pub fn topic_marks(topic: &str) -> Vec<&str> {
    words(topic)
        .filter(|w| {
            let acronym = w.chars().count() >= 3
                && w.chars().all(|c| c.is_uppercase() || c.is_ascii_digit())
                && !GENERIC.contains(w);
            acronym || is_code(w)
        })
        .collect()
}

/// Four characters or more, as shorter codes pick out too little.
pub fn name_marks(name: &str) -> Vec<String> {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    words(stem)
        .filter(|w| w.chars().count() >= 4 && is_code(w))
        .map(str::to_lowercase)
        .collect()
}

pub fn share_mark(a: &str, b: &str) -> bool {
    let b_marks = topic_marks(b);
    topic_marks(a)
        .iter()
        .any(|m| b_marks.iter().any(|n| m.to_lowercase() == n.to_lowercase()))
}

pub fn shared_mark<'a>(topics: &[&'a str]) -> Option<&'a str> {
    let (first, rest) = topics.split_first()?;
    topic_marks(first).into_iter().find(|m| {
        rest.iter().all(|t| {
            topic_marks(t)
                .iter()
                .any(|n| n.to_lowercase() == m.to_lowercase())
        })
    })
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// Ordinals ("2nd", "10th") mix letters and digits without being codes.
fn is_code(word: &str) -> bool {
    let has_letter = word.chars().any(char::is_alphabetic);
    let has_digit = word.chars().any(|c| c.is_ascii_digit());
    let lower = word.to_lowercase();
    let ordinal = ["st", "nd", "rd", "th"].iter().any(|s| {
        lower
            .strip_suffix(s)
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    });
    has_letter && has_digit && !ordinal
}
