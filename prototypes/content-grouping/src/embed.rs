//! Sentence embeddings from a local ONNX model: tokenize, run, pool (the
//! first token or the mean over the attention mask), L2-normalize. Only the
//! model download touches the network; file contents never leave the
//! machine.

use crate::download;
use crate::extract::Content;
use ndarray::{Array2, Axis};
use ort::session::Session;
use ort::value::Value;
use std::path::{Path, PathBuf};
use tokenizers::{PaddingParams, Tokenizer, TruncationParams};

/// BAAI/bge-m3 exported by transformers.js: multilingual (Vietnamese
/// included), 1024 dimensions, about 2.3 GB.
pub const DEFAULT_MODEL: &str = "Xenova/bge-m3";
/// (path in the model repository, file name in the cache). The weights sit
/// in `model.onnx_data` next to the graph, which ONNX Runtime finds by name.
const MODEL_FILES: &[(&str, &str)] = &[
    ("tokenizer.json", "tokenizer.json"),
    ("onnx/model.onnx", "model.onnx"),
    ("onnx/model.onnx_data", "model.onnx_data"),
];
/// Tokens per text. bge-m3 takes up to 8,192, but attention cost grows with
/// the square of the length, and `--max-chars` of text fits in 512.
const MAX_TOKENS: usize = 512;
/// Small batches keep ONNX Runtime's intermediate buffers small.
const BATCH: usize = 8;
/// Each extra thread adds its own scratch buffers; past a few, little is gained.
const MAX_THREADS: usize = 4;

pub type Vector = Vec<f32>;

/// How a text's token vectors become one vector; each model is trained for
/// one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Pooling {
    /// The first (`[CLS]`/`<s>`) token: bge models.
    Cls,
    /// The mean over the real tokens: most sentence-transformers models, e5.
    Mean,
}

pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
    needs_token_type_ids: bool,
    pooling: Pooling,
    /// Put before every text, e.g. `query: ` for e5 models.
    prefix: String,
    pub model_label: String,
}

/// Where the default model is cached when `--model-dir` isn't given.
pub fn default_model_dir() -> Option<PathBuf> {
    Some(download::cache_root()?.join(DEFAULT_MODEL.replace('/', "--")))
}

/// Downloads the default model's files into `dir`, once.
pub fn ensure_default_model(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("can't create model cache {}: {e}", dir.display()))?;
    for (remote, local) in MODEL_FILES {
        let dest = dir.join(local);
        if !dest.exists() {
            download::fetch(&format!("https://huggingface.co/{DEFAULT_MODEL}/resolve/main/{remote}"), &dest)?;
        }
    }
    Ok(())
}

impl Embedder {
    /// Loads `model.onnx` (and `model.onnx_data`, if the graph keeps its
    /// weights there) and `tokenizer.json` from `dir`: any
    /// sentence-transformers model exported to ONNX whose output is
    /// `last_hidden_state`.
    pub fn load(dir: &Path, pooling: Pooling, prefix: String, model_label: String) -> Result<Self, String> {
        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| format!("tokenizer: {e}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams { max_length: MAX_TOKENS, ..Default::default() }))
            .map_err(|e| format!("tokenizer: {e}"))?;
        // Pad with the model's own pad token: RoBERTa-style models (bge-m3)
        // use `<pad>` = 1 and treat id 0 as the start token.
        let mut padding = PaddingParams::default();
        if let Some((token, id)) = ["[PAD]", "<pad>"].iter().find_map(|t| Some((*t, tokenizer.token_to_id(t)?))) {
            padding.pad_token = token.to_string();
            padding.pad_id = id;
        }
        tokenizer.with_padding(Some(padding));
        let threads = std::thread::available_parallelism().map_or(MAX_THREADS, |n| n.get().min(MAX_THREADS));
        let environment = ort::init().build().map_err(|e| format!("ONNX Runtime: {e}"))?;
        // No memory arena and no per-shape memory pattern: both keep the
        // largest buffers ever needed for the whole run.
        let session = Session::builder(&environment)
            .and_then(|b| {
                b.with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()])
                    .map_err(Into::into)
            })
            .and_then(|b| b.with_memory_pattern(false).map_err(Into::into))
            .and_then(|b| b.with_intra_threads(threads).map_err(Into::into))
            .and_then(|mut b| b.commit_from_file(dir.join("model.onnx")))
            .map_err(|e: ort::Error| format!("ONNX model: {e}"))?;
        let needs_token_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");
        Ok(Self { session, tokenizer, needs_token_type_ids, pooling, prefix, model_label })
    }

    /// One vector per file. The name and the content are embedded apart
    /// and averaged, `name_weight` to the name: in one text the name would
    /// be a few tokens among hundreds, and a file read by name only would be
    /// a short title compared with pages of text. A name-only file gets its
    /// name's vector.
    pub fn embed_contents(&mut self, contents: &[Content], name_weight: f32) -> Result<Vec<Vector>, String> {
        let names: Vec<String> = contents.iter().map(|c| c.name.clone()).collect();
        let with_body: Vec<usize> = (0..contents.len()).filter(|&i| !contents[i].body.is_empty()).collect();
        let bodies: Vec<String> = with_body.iter().map(|&i| contents[i].body.clone()).collect();
        let mut vectors = self.embed(&names)?;
        for (&i, body) in with_body.iter().zip(self.embed(&bodies)?) {
            let mixed = vectors[i].iter().zip(&body).map(|(n, b)| name_weight * n + (1.0 - name_weight) * b).collect();
            vectors[i] = normalized(mixed);
        }
        Ok(vectors)
    }

    /// Texts are batched by length, so a batch pads little; the vectors come
    /// back in the input's order.
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        let mut order: Vec<usize> = (0..texts.len()).collect();
        order.sort_by_key(|&i| texts[i].len());
        let mut out = vec![Vector::new(); texts.len()];
        for batch in order.chunks(BATCH) {
            let batch_texts: Vec<String> = batch.iter().map(|&i| format!("{}{}", self.prefix, texts[i])).collect();
            for (&i, v) in batch.iter().zip(self.embed_batch(&batch_texts)?) {
                out[i] = v;
            }
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

        let mut vectors = Vec::with_capacity(rows);
        for (r, tokens) in hidden.axis_iter(Axis(0)).enumerate() {
            if self.pooling == Pooling::Cls {
                vectors.push(normalized(tokens.index_axis(Axis(0), 0).iter().copied().collect()));
                continue;
            }
            // Mean over the real (unpadded) tokens of the text.
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
