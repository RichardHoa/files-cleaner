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
