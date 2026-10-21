//! A small local LLM that reads each file once and writes a short card: the
//! topic it belongs to, what kind of document it is, and one sentence about
//! it. Files worded differently but about the same course, project or
//! organization get the same topic, which is what groups them. Runs on the
//! CPU, or the GPU through Metal on macOS; file contents never leave the
//! machine.

use crate::download;
use crate::extract::Content;
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights;
use std::path::{Path, PathBuf};
use tokenizers::Tokenizer;

/// Qwen2.5 1.5B Instruct, 4-bit (about 1.1 GB): reads and writes Vietnamese
/// and English.
pub const DEFAULT_LLM: &str = "Qwen/Qwen2.5-1.5B-Instruct";
/// (URL, file name in the cache).
const LLM_FILES: &[(&str, &str)] = &[
    (
        "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
        "model.gguf",
    ),
    ("https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct/resolve/main/tokenizer.json", "tokenizer.json"),
];
/// Characters of content the LLM reads. Fewer than the embedding saw: the
/// prompt is processed token by token on the CPU, and Vietnamese takes more
/// tokens per character than English.
const LLM_CHARS: usize = 1200;
/// A card is three short fields; anything longer is the model rambling.
const MAX_NEW_TOKENS: usize = 120;
const STOP_TOKENS: &[&str] = &["<|im_end|>", "<|endoftext|>"];

/// What the LLM says about one file. Every field may be empty.
#[derive(Debug, Clone, Default, PartialEq)]
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
        let key = self.topic.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        (!key.is_empty()).then_some(key)
    }

    /// What gets embedded.
    pub fn text(&self) -> String {
        [&self.topic, &self.kind, &self.summary]
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(". ")
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
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else { return Card::default() };
    let field = |k: &str| value.get(k).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    Card { topic: field("topic"), kind: field("type"), summary: field("summary") }
}

/// Where the default LLM is cached when `--llm-dir` isn't given.
pub fn default_llm_dir() -> Option<PathBuf> {
    Some(download::cache_root()?.join(DEFAULT_LLM.replace('/', "--")))
}

/// Downloads the default LLM's files into `dir`, once.
pub fn ensure_default_llm(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("can't create LLM cache {}: {e}", dir.display()))?;
    for (url, local) in LLM_FILES {
        let dest = dir.join(local);
        if !dest.exists() {
            download::fetch(url, &dest)?;
        }
    }
    Ok(())
}

pub struct Llm {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    stop: Vec<u32>,
}

impl Llm {
    /// Loads `model.gguf` (a Qwen2 or Qwen2.5 GGUF) and `tokenizer.json`
    /// from `dir`. Uses Metal when it's there, unless `cpu`.
    pub fn load(dir: &Path, cpu: bool) -> Result<Self, String> {
        let device = if cpu { Device::Cpu } else { Device::metal_if_available(0).map_err(|e| e.to_string())? };
        let path = dir.join("model.gguf");
        let mut file = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let content = gguf_file::Content::read(&mut file).map_err(|e| format!("{}: {e}", path.display()))?;
        let model = ModelWeights::from_gguf(content, &mut file, &device).map_err(|e| format!("LLM: {e}"))?;
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| format!("LLM tokenizer: {e}"))?;
        let stop = STOP_TOKENS.iter().filter_map(|t| tokenizer.token_to_id(t)).collect();
        Ok(Self { model, tokenizer, device, stop })
    }

    pub fn device_label(&self) -> &'static str {
        if self.device.is_metal() {
            "Metal GPU"
        } else {
            "CPU"
        }
    }

    pub fn card(&mut self, content: &Content) -> Result<Card, String> {
        Ok(parse_card(&self.complete(&prompt(content))?))
    }

    /// Greedy decoding, so the same file always gets the same card.
    fn complete(&mut self, prompt: &str) -> Result<String, String> {
        let err = |e: candle_core::Error| e.to_string();
        let tokens = self.tokenizer.encode(prompt, false).map_err(|e| e.to_string())?.get_ids().to_vec();
        let mut sampler = LogitsProcessor::from_sampling(0, Sampling::ArgMax);
        // Position 0 starts a fresh KV cache, so no file sees the one before.
        let input = Tensor::new(tokens.as_slice(), &self.device).and_then(|t| t.unsqueeze(0)).map_err(err)?;
        let logits = self.model.forward(&input, 0).and_then(|l| l.squeeze(0)).map_err(err)?;
        let mut next = sampler.sample(&logits).map_err(err)?;
        let mut out = Vec::new();
        for i in 0..MAX_NEW_TOKENS {
            if self.stop.contains(&next) {
                break;
            }
            out.push(next);
            let input = Tensor::new(&[next], &self.device).and_then(|t| t.unsqueeze(0)).map_err(err)?;
            let logits = self.model.forward(&input, tokens.len() + i).and_then(|l| l.squeeze(0)).map_err(err)?;
            next = sampler.sample(&logits).map_err(err)?;
        }
        self.tokenizer.decode(&out, true).map_err(|e| e.to_string())
    }
}

const SYSTEM: &str = "You label files so they can be sorted into folders. Reply with one JSON object and nothing else.";

/// Qwen's chat template, written out.
fn prompt(content: &Content) -> String {
    let body = if content.body.is_empty() {
        "(none: only the file name is known; leave topic empty unless the name makes it obvious)".to_string()
    } else {
        content.body.chars().take(LLM_CHARS).collect()
    };
    let user = format!(
        "File name: {}\n\
         Start of the file:\n\"\"\"\n{body}\n\"\"\"\n\n\
         Reply with a JSON object with these keys:\n\
         \"topic\": the specific course, project, organization, person, product or subject this file belongs to, \
         in 1 to 4 words, in the file's language (e.g. \"CS101\", \"IELTS Writing\", \"Công ty ABC\"); \"\" if unclear.\n\
         \"type\": what kind of document it is, in 1 to 3 English words (e.g. \"invoice\", \"lecture notes\", \"resume\").\n\
         \"summary\": one short sentence on what the file is about, in the file's language.",
        content.name
    );
    format!("<|im_start|>system\n{SYSTEM}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_json_out_of_a_chatty_reply() {
        let reply = "Here you go:\n```json\n{\"topic\": \"Tiếng Anh IELTS\", \"type\": \"essay\", \"summary\": \"Bài luận.\"}\n```";
        let card = parse_card(reply);
        assert_eq!(card.topic, "Tiếng Anh IELTS");
        assert_eq!(card.kind, "essay");
        assert_eq!(card.topic_key().as_deref(), Some("tiếng anh ielts"));
    }

    #[test]
    fn a_reply_without_json_is_an_empty_card() {
        assert_eq!(parse_card("I can't tell."), Card::default());
        assert_eq!(parse_card("{\"topic\": \"\"}").topic_key(), None);
    }
}
