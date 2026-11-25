//! One `.gguf` file, never a repo: a repo holds many quantizations.

use serde::Serialize;

use crate::download_hosts::DOWNLOAD_HOST;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLink {
    pub url: String,
    pub file_name: String,
}

const NOT_ONE_FILE: &str = "Paste the link to one .gguf file.";

pub fn describing_model_link(link: &str) -> Result<ModelLink, String> {
    let link = link.trim();
    // A pasted link may name Hugging Face's other hosts, where downloads can't
    // start.
    let rest = link
        .strip_prefix("https://")
        .and_then(|rest| {
            [DOWNLOAD_HOST, "www.huggingface.co", "hf.co"]
                .iter()
                .find_map(|host| rest.strip_prefix(host)?.strip_prefix('/'))
        })
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
            "https://{DOWNLOAD_HOST}/{owner}/{repo}/resolve/{revision}/{}",
            file.join("/")
        ),
        file_name: file_name.to_string(),
    })
}
