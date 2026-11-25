//! Keeps network access in one file. The build script isn't checked: it
//! downloads at build time only.

use std::fs;
use std::path::{Path, PathBuf};

const NETWORK: [&str; 14] = [
    "ureq",
    "reqwest",
    "hyper",
    "isahc",
    "attohttpc",
    "minreq",
    "surf",
    "curl",
    "ehttp",
    "std::net",
    "tokio::net",
    "TcpStream",
    "UdpSocket",
    "socket2",
];

const DOWNLOADS: &str = "src/downloads.rs";

fn rust_files(folder: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

fn has_word(text: &str, name: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(name).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(word)
            && !text[at + name.len()..].chars().next().is_some_and(word)
    })
}

#[test]
fn only_the_downloads_module_uses_the_network() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for crate_source in ["src", "core/src", "grouping/src"] {
        rust_files(&app.join(crate_source), &mut files);
    }
    assert!(files.iter().any(|file| file.ends_with(DOWNLOADS)));

    let offending: Vec<String> = files
        .iter()
        .filter(|file| !file.ends_with(DOWNLOADS))
        .flat_map(|file| {
            let text = fs::read_to_string(file).unwrap();
            NETWORK
                .iter()
                .filter(move |name| has_word(&text, name))
                .map(move |name| format!("{} uses {name}", file.display()))
        })
        .collect();

    assert!(offending.is_empty(), "{offending:#?}");
}
