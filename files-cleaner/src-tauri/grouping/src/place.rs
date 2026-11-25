//! Embeddings can't tell that "English Collocation" is English study like
//! "IELTS Writing"; the LLM can.

use serde::{Deserialize, Serialize};

use crate::card::{Card, Chat};

#[derive(Debug, Clone)]
pub struct Question {
    pub file_name: String,
    pub card: Card,
    pub folders: Vec<Choice>,
}

#[derive(Debug, Clone)]
pub struct Choice {
    pub name: String,
    pub topics: Vec<String>,
    /// So a file can be matched by its name too.
    pub files: Vec<String>,
    /// Rather than a Suggested Folder.
    pub existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub folder: Option<String>,
}

/// Keeps the prompt short.
pub const CHOICES: usize = 8;
/// Keeps the prompt short.
const TOPICS: usize = 6;
/// Keeps the prompt short.
const FILES: usize = 4;
pub const MAX_NEW_TOKENS: usize = 24;

const SYSTEM: &str = "You sort files into folders. Reply with one folder name and nothing else.";

/// Identifies the question in the store, whichever model's template it is asked
/// with.
pub fn prompt(question: &Question) -> String {
    chat(question).chatml()
}

pub fn chat(question: &Question) -> Chat {
    let card = &question.card;
    // "none" goes last: small models lean towards the first choice.
    let mut folders = String::new();
    for choice in &question.folders {
        let mut about = Vec::new();
        if choice.existing {
            about.push("an existing folder".to_string());
        }
        let topics: Vec<&str> = choice
            .topics
            .iter()
            .take(TOPICS)
            .map(String::as_str)
            .collect();
        if !topics.is_empty() {
            about.push(format!("topics: {}", topics.join("; ")));
        }
        let files: Vec<&str> = choice
            .files
            .iter()
            .take(FILES)
            .map(String::as_str)
            .collect();
        if !files.is_empty() {
            about.push(format!("files: {}", files.join(", ")));
        }
        folders.push_str(&format!("- {} ({})\n", label(choice), about.join("; ")));
    }
    folders.push_str("- none (it fits none of these)\n");
    let user = format!(
        "A file:\nName: {}\nTopic: {}\nType: {}\nSummary: {}\n\nFolders:\n{folders}\n\
         Which folder would a person keep this file in? Judge by what the file is about. \
         Choose a folder when the file is about the same course, project, organization or subject as the files in it, \
         or is study material for that subject or exam \
         (a calculus lecture goes with calculus homework; vocabulary notes go with language exam papers). \
         Being written in the same language, or being something one could read for practice, is not enough: \
         a report on politics or health does not go with language exam papers. \
         If no folder is about the file's subject, reply \"none\". \
         Reply with the folder name exactly as written above, or \"none\".",
        question.file_name, card.topic, card.kind, card.summary
    );
    Chat {
        system: SYSTEM,
        user,
    }
}

/// Small models drop the slash, so a name without it counts when only one
/// folder has it.
pub fn parse(reply: &str, question: &Question) -> Option<usize> {
    let bare = |s: &str| {
        s.trim()
            .trim_matches(|c: char| "\"'`*.-: ".contains(c))
            .to_lowercase()
    };
    let reply = bare(reply.lines().next().unwrap_or(""));
    let exact = question
        .folders
        .iter()
        .position(|choice| bare(&label(choice)) == reply);
    let named: Vec<usize> = (0..question.folders.len())
        .filter(|&i| bare(&question.folders[i].name) == reply)
        .collect();
    exact.or(match named[..] {
        [only] => Some(only),
        _ => None,
    })
}

/// The slash keeps an existing folder apart from a Suggested Folder of the same
/// name.
pub fn label(choice: &Choice) -> String {
    if choice.existing {
        format!("{}/", choice.name)
    } else {
        choice.name.clone()
    }
}
