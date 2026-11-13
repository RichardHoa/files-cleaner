//! The Hugging Face link to the model the user picked to describe files
//! (ADR 0009): only a link to one `.gguf` file, never a repo, since a repo
//! holds many quantizations.

use serde::Serialize;

/// A link to one GGUF file on Hugging Face, as the app downloads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLink {
    /// The file's download link: its `/resolve/` link.
    pub url: String,
    /// The file's name, which it is saved as.
    pub file_name: String,
}

const NOT_ONE_FILE: &str = "Paste the link to one .gguf file.";

/// The download link for `link`, a Hugging Face `/blob/` or `/resolve/`
/// link to one `.gguf` file, or why it can't be used.
pub fn describing_model_link(link: &str) -> Result<ModelLink, String> {
    let link = link.trim();
    let rest = [
        "https://huggingface.co/",
        "https://www.huggingface.co/",
        "https://hf.co/",
    ]
    .iter()
    .find_map(|host| link.strip_prefix(host))
    .ok_or_else(|| {
        "Paste a Hugging Face link, starting with https://huggingface.co/.".to_string()
    })?;
    // `?download=true` and the like change nothing about which file it is.
    let path = rest.split(['?', '#']).next().unwrap_or("");
    let parts: Vec<&str> = path.split('/').collect();
    let [owner, repo, "blob" | "resolve", revision, ref file @ ..] = parts[..] else {
        return Err(NOT_ONE_FILE.to_string());
    };
    let file_name = file.last().copied().unwrap_or("");
    let safe = |part: &str| {
        !part.is_empty()
            && !part.starts_with('.')
            && !part.contains(['\\', ':', '%'])
            && !part.chars().any(char::is_control)
    };
    if !file_name.to_lowercase().ends_with(".gguf")
        || ![owner, repo, revision]
            .into_iter()
            .chain(file.iter().copied())
            .all(safe)
    {
        return Err(NOT_ONE_FILE.to_string());
    }
    Ok(ModelLink {
        url: format!(
            "https://huggingface.co/{owner}/{repo}/resolve/{revision}/{}",
            file.join("/")
        ),
        file_name: file_name.to_string(),
    })
}
