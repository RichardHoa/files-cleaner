//! Placing a file left out of every Suggested Folder: the LLM is shown the
//! file's card and the folders nearest it, each with the topics it holds,
//! and picks one or none. Embeddings can't tell that "English Collocation"
//! is English study like "IELTS Writing"; the LLM can.

use serde::{Deserialize, Serialize};

use crate::card::Card;

/// What the LLM is asked about one file.
#[derive(Debug, Clone)]
pub struct Question {
    /// The file's own name.
    pub file_name: String,
    pub card: Card,
    /// The folders it may go into, most like it first.
    pub folders: Vec<Choice>,
}

/// A folder a file may go into.
#[derive(Debug, Clone)]
pub struct Choice {
    pub name: String,
    /// The topics of the files in it, each once.
    pub topics: Vec<String>,
    /// An existing subfolder of the Scan Root, rather than a Suggested Folder.
    pub existing: bool,
}

/// The answer the store keeps: the folder's label, or none.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub folder: Option<String>,
}

/// The most folders a file is offered, so the prompt stays short.
pub const CHOICES: usize = 8;
/// The most topics shown for each folder.
const TOPICS: usize = 6;
/// Tokens a reply takes: one folder name.
pub const MAX_NEW_TOKENS: usize = 24;

const SYSTEM: &str = "You sort files into folders. Reply with one folder name and nothing else.";

/// What the LLM is asked, in Qwen's chat template, written out.
pub fn prompt(question: &Question) -> String {
    let card = &question.card;
    let mut folders = String::from("- none (it fits none of these)\n");
    for choice in &question.folders {
        let topics: Vec<&str> = choice.topics.iter().take(TOPICS).map(String::as_str).collect();
        let about = match (choice.existing, topics.is_empty()) {
            (true, true) => "an existing folder".to_string(),
            (true, false) => format!("an existing folder; holds: {}", topics.join("; ")),
            (false, _) => format!("holds: {}", topics.join("; ")),
        };
        folders.push_str(&format!("- {} ({about})\n", label(choice)));
    }
    let user = format!(
        "A file:\nName: {}\nTopic: {}\nType: {}\nSummary: {}\n\nFolders:\n{folders}\n\
         Would a person keep this file in one of these folders? Pick one only if the file is clearly about \
         the same course, project, organization or subject as the files it holds, or part of it. \
         Reply with the folder name exactly as written above, or \"none\".",
        question.file_name, card.topic, card.kind, card.summary
    );
    format!("<|im_start|>system\n{SYSTEM}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n")
}

/// The folder the reply names, by its place in `question.folders`; `None`
/// for "none" or a reply naming no folder. A name without its slash counts
/// when only one folder has it.
pub fn parse(reply: &str, question: &Question) -> Option<usize> {
    let bare = |s: &str| s.trim().trim_matches(|c: char| "\"'`*.-: ".contains(c)).to_lowercase();
    let reply = bare(reply.lines().next().unwrap_or(""));
    let exact = question.folders.iter().position(|choice| bare(&label(choice)) == reply);
    let named: Vec<usize> = (0..question.folders.len()).filter(|&i| bare(&question.folders[i].name) == reply).collect();
    exact.or(match named[..] {
        [only] => Some(only),
        _ => None,
    })
}

/// How a folder is written in the prompt: an existing one with a slash, so
/// it never reads as a Suggested Folder of the same name.
pub fn label(choice: &Choice) -> String {
    if choice.existing {
        format!("{}/", choice.name)
    } else {
        choice.name.clone()
    }
}
