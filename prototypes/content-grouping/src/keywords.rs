//! Words of file names, for the baseline grouper.

use std::collections::HashSet;

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

pub fn title_case(word: &str) -> String {
    if word.chars().any(|c| c.is_numeric()) {
        return word.to_uppercase();
    }
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}
