//! A file's card: what the LLM says it belongs to, what kind of document it
//! is, and one sentence about it. Files worded differently but about the
//! same course, project or organization get the same topic, which is what
//! groups them.

use serde::{Deserialize, Serialize};

use crate::read::{Content, READER_VERSION};

/// What the LLM says about one file. Every field may be empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Card {
    /// The course, project, organization or subject, in the file's language.
    pub topic: String,
    /// Invoice, lecture notes, resume, …
    pub kind: String,
    pub summary: String,
}

impl Card {
    /// The topic, lowercased with whitespace collapsed: files with equal
    /// keys always group. `None` when the LLM gave no topic.
    pub fn topic_key(&self) -> Option<String> {
        let key = self
            .topic
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        (!key.is_empty()).then_some(key)
    }

    /// What gets embedded: the card, or `name` when the card is empty.
    pub fn text(&self, name: &str) -> String {
        let text = [&self.topic, &self.kind, &self.summary]
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(". ");
        if text.is_empty() {
            name.to_string()
        } else {
            text
        }
    }
}

/// The first `{…}` in the reply, read as a card. Small models sometimes add
/// prose or a code fence around it; a reply with no usable JSON is an empty
/// card.
pub fn parse_card(reply: &str) -> Card {
    let json = match (reply.find('{'), reply.rfind('}')) {
        (Some(a), Some(b)) if a < b => &reply[a..=b],
        _ => return Card::default(),
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Card::default();
    };
    let field = |k: &str| {
        value
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    Card {
        topic: field("topic"),
        kind: field("type"),
        summary: field("summary"),
    }
}

/// Characters of content the LLM reads: fewer than are read, since
/// Vietnamese takes more tokens per character than English.
const LLM_CHARS: usize = 1200;

const SYSTEM: &str = "You label files so they can be sorted into folders. Reply with one JSON object and nothing else.";

/// What the LLM is asked for, after the file.
const INSTRUCTIONS: &str = "Reply with a JSON object with these keys:\n\
\"topic\": the course, project, organization, person, product or subject this file belongs to, \
in 1 to 4 words, in the file's language. Name it as a person would name a folder for it, so related files \
get the same topic: when the file gives a full name and a code, use the full name (\"Linear Algebra\", not \"MATH221\"); \
use the broad subject rather than one part of it (\"TOEFL\", not \"TOEFL Listening Section 2\"). \
Look for it in the title, headers and first lines as well as the file name. \"\" if unclear.\n\
\"type\": what kind of document it is, in 1 to 3 English words (e.g. \"invoice\", \"lecture notes\", \"resume\").\n\
\"summary\": one short sentence on what the file is about, in the file's language.";

/// Identifies the prompt and how files are read for it, so cards written
/// with an earlier one aren't used: changing either reads every file again.
pub fn prompt_id() -> String {
    let id =
        blake3::hash(format!("{SYSTEM}\n{INSTRUCTIONS}\n{LLM_CHARS}\n{READER_VERSION}").as_bytes());
    id.to_hex()[..8].to_string()
}

/// The card prompt in Qwen's chat template, written out.
pub fn prompt(content: &Content) -> String {
    let body = if content.body.is_empty() {
        "(none: only the file name is known; leave topic empty unless the name makes it obvious)"
            .to_string()
    } else {
        content.body.chars().take(LLM_CHARS).collect()
    };
    let user = format!(
        "File name: {}\n\
         Start of the file:\n\"\"\"\n{body}\n\"\"\"\n\n{INSTRUCTIONS}",
        content.name
    );
    format!("<|im_start|>system\n{SYSTEM}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n")
}
