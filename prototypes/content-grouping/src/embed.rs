//! Sentence embeddings from a local ONNX model: tokenize, run, mean-pool over
//! the attention mask, L2-normalize. Only the model download touches the
//! network; file contents never leave the machine.

use ndarray::{Array2, Axis};
use ort::session::Session;
use ort::value::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokenizers::{PaddingParams, Tokenizer, TruncationParams};

pub const DEFAULT_MODEL: &str = "Qdrant/all-MiniLM-L6-v2-onnx";
const MODEL_FILES: &[&str] = &["model.onnx", "tokenizer.json"];
/// Tokens per text; all-MiniLM-L6-v2 was trained on 256.
const MAX_TOKENS: usize = 256;
const BATCH: usize = 32;

pub type Vector = Vec<f32>;

pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
    needs_token_type_ids: bool,
    pub model_label: String,
}

/// Where the default model is cached when `--model-dir` isn't given.
pub fn default_model_dir() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("files-cleaner-content-grouping").join(DEFAULT_MODEL.replace('/', "--")))
}

/// Downloads the default model's files into `dir` with the system `curl`
/// (shipped with macOS, Windows 10+ and most Linux distributions), once.
pub fn ensure_default_model(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("can't create model cache {}: {e}", dir.display()))?;
    for file in MODEL_FILES {
        let dest = dir.join(file);
        if dest.exists() {
            continue;
        }
        let url = format!("https://huggingface.co/{DEFAULT_MODEL}/resolve/main/{file}");
        eprintln!("Downloading {url}");
        let partial = dir.join(format!("{file}.part"));
        let status = Command::new("curl")
            .args(["--fail", "--location", "--silent", "--show-error", "--output"])
            .arg(&partial)
            .arg(&url)
            .status()
            .map_err(|e| format!("can't run curl to download the model ({e}); download {url} into {} yourself", dir.display()))?;
        if !status.success() {
            return Err(format!("curl failed downloading {url}"));
        }
        std::fs::rename(&partial, &dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl Embedder {
    /// Loads `model.onnx` and `tokenizer.json` from `dir`: any
    /// sentence-transformers model exported to ONNX whose output is
    /// `last_hidden_state`.
    pub fn load(dir: &Path, model_label: String) -> Result<Self, String> {
        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| format!("tokenizer: {e}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams { max_length: MAX_TOKENS, ..Default::default() }))
            .map_err(|e| format!("tokenizer: {e}"))?;
        tokenizer.with_padding(Some(PaddingParams::default()));
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let environment = ort::init().build().map_err(|e| format!("ONNX Runtime: {e}"))?;
        let session = Session::builder(&environment)
            .and_then(|b| b.with_intra_threads(threads).map_err(Into::into))
            .and_then(|mut b| b.commit_from_file(dir.join("model.onnx")))
            .map_err(|e: ort::Error| format!("ONNX model: {e}"))?;
        let needs_token_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");
        Ok(Self { session, tokenizer, needs_token_type_ids, model_label })
    }

    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        let mut out = Vec::with_capacity(texts.len());
        for batch in texts.chunks(BATCH) {
            out.extend(self.embed_batch(batch)?);
        }
        Ok(out)
    }

    fn embed_batch(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        let encodings =
            self.tokenizer.encode_batch(texts.iter().map(String::as_str).collect(), true).map_err(|e| e.to_string())?;
        let rows = encodings.len();
        let cols = encodings.iter().map(|e| e.len()).max().unwrap_or(0);
        let matrix = |field: fn(&tokenizers::Encoding) -> &[u32]| {
            Array2::from_shape_fn((rows, cols), |(r, c)| field(&encodings[r]).get(c).copied().unwrap_or(0) as i64)
        };
        let ids = matrix(|e| e.get_ids());
        let mask = matrix(|e| e.get_attention_mask());
        let types = matrix(|e| e.get_type_ids());

        let value = |a: Array2<i64>| Value::from_array(a).map_err(|e| e.to_string());
        let mut inputs = ort::inputs!["input_ids" => value(ids)?, "attention_mask" => value(mask.clone())?];
        if self.needs_token_type_ids {
            inputs.push(("token_type_ids".into(), value(types)?.into()));
        }
        let outputs = self.session.run(inputs).map_err(|e| e.to_string())?;
        let hidden = outputs["last_hidden_state"].try_extract_array::<f32>().map_err(|e| e.to_string())?;

        // Mean over the real (unpadded) tokens of each text.
        let mut vectors = Vec::with_capacity(rows);
        for (r, tokens) in hidden.axis_iter(Axis(0)).enumerate() {
            let mut sum = vec![0f32; tokens.shape()[1]];
            let mut count = 0f32;
            for (c, token) in tokens.axis_iter(Axis(0)).enumerate() {
                if mask[(r, c)] == 1 {
                    count += 1.0;
                    sum.iter_mut().zip(token.iter()).for_each(|(s, v)| *s += v);
                }
            }
            sum.iter_mut().for_each(|s| *s /= count.max(1.0));
            vectors.push(normalized(sum));
        }
        Ok(vectors)
    }
}

pub fn normalized(mut v: Vector) -> Vector {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The normalized mean of normalized vectors.
pub fn centroid<'a>(vectors: impl IntoIterator<Item = &'a Vector>) -> Option<Vector> {
    let mut sum: Option<Vector> = None;
    for v in vectors {
        let s = sum.get_or_insert_with(|| vec![0.0; v.len()]);
        s.iter_mut().zip(v).for_each(|(s, x)| *s += x);
    }
    sum.map(normalized)
}
