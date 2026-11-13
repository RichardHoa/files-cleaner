use files_cleaner_core::describing_model_link;

#[test]
fn a_blob_link_to_a_gguf_file_becomes_its_download_link() {
    let link = describing_model_link(
        "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/blob/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
    )
    .unwrap();

    assert_eq!(
        link.url,
        "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf"
    );
    assert_eq!(link.file_name, "Llama-3.2-3B-Instruct-Q4_K_M.gguf");
}

#[test]
fn a_repo_link_is_refused() {
    let refused =
        describing_model_link("https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF")
            .unwrap_err();

    assert_eq!(refused, "Paste the link to one .gguf file.");
}

const FILE: &str = "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf";

#[test]
fn a_resolve_link_is_kept_as_it_is() {
    assert_eq!(describing_model_link(FILE).unwrap().url, FILE);
}

#[test]
fn a_query_string_or_fragment_is_dropped() {
    for link in [
        format!("{FILE}?download=true"),
        format!("{FILE}#top"),
        format!("  {FILE}?download=true#top \n"),
    ] {
        assert_eq!(describing_model_link(&link).unwrap().url, FILE, "{link}");
    }
}

#[test]
fn hf_co_and_www_links_become_huggingface_co_links() {
    for host in ["https://hf.co/", "https://www.huggingface.co/"] {
        let link = FILE.replace("https://huggingface.co/", host);
        assert_eq!(describing_model_link(&link).unwrap().url, FILE, "{link}");
    }
}

#[test]
fn a_file_in_a_subfolder_keeps_its_path_and_is_saved_by_its_name() {
    let link = describing_model_link(
        "https://huggingface.co/unsloth/Qwen3-8B-GGUF/blob/main/Q4_K_M/Qwen3-8B-Q4_K_M.gguf",
    )
    .unwrap();

    assert_eq!(
        link.url,
        "https://huggingface.co/unsloth/Qwen3-8B-GGUF/resolve/main/Q4_K_M/Qwen3-8B-Q4_K_M.gguf"
    );
    assert_eq!(link.file_name, "Qwen3-8B-Q4_K_M.gguf");
}

#[test]
fn repo_tree_and_folder_links_are_refused() {
    for link in [
        "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/",
        "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/tree/main",
        "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/tree/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main",
        "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/",
    ] {
        assert_eq!(
            describing_model_link(link).unwrap_err(),
            "Paste the link to one .gguf file.",
            "{link}"
        );
    }
}

#[test]
fn a_trailing_slash_after_the_file_name_is_refused() {
    assert!(describing_model_link(&format!("{FILE}/")).is_err());
}

#[test]
fn files_that_arent_gguf_are_refused() {
    for name in ["model.safetensors", "README.md", "model.gguf.part", "gguf"] {
        let link = FILE.replace("Llama-3.2-3B-Instruct-Q4_K_M.gguf", name);
        assert!(describing_model_link(&link).is_err(), "{link}");
    }
}

#[test]
fn encoded_characters_and_dot_names_are_refused_so_no_path_can_escape() {
    for name in [
        "Llama%20Q4.gguf",
        "..%2F..%2Fevil.gguf",
        "..",
        ".hidden.gguf",
        "a\\b.gguf",
        "C:evil.gguf",
    ] {
        let link = FILE.replace("Llama-3.2-3B-Instruct-Q4_K_M.gguf", name);
        assert!(describing_model_link(&link).is_err(), "{link}");
    }
    assert!(describing_model_link(&FILE.replace("/main/", "/../")).is_err());
}

#[test]
fn links_to_other_sites_or_over_http_are_refused() {
    for link in [
        FILE.replace("https://", "http://"),
        FILE.replace("huggingface.co", "huggingface.co.evil.com"),
        FILE.replace("huggingface.co", "huggingface.co@evil.com"),
        FILE.replace("huggingface.co", "evilhuggingface.co"),
        FILE.replace("huggingface.co", "huggingface.co:8443"),
    ] {
        assert_eq!(
            describing_model_link(&link).unwrap_err(),
            "Paste a Hugging Face link, starting with https://huggingface.co/.",
            "{link}"
        );
    }
}
