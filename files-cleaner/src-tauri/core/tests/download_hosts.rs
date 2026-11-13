use files_cleaner_core::{check_download, follow_redirect, Hop};

const MODEL: &str = "https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/model.gguf";

#[test]
fn a_hugging_face_link_is_allowed_as_the_first_request() {
    assert_eq!(check_download(MODEL, Hop::First), Ok(()));
}

#[test]
fn plain_http_is_refused_on_every_hop() {
    let http = "http://huggingface.co/Qwen/model.gguf";

    assert_eq!(
        check_download(http, Hop::First),
        Err("the link isn't HTTPS".to_string())
    );
    assert_eq!(
        check_download(http, Hop::Redirect),
        Err("the link isn't HTTPS".to_string())
    );
}

#[test]
fn other_hosts_are_refused() {
    assert_eq!(
        check_download("https://github.com/x/model.gguf", Hop::First),
        Err("github.com isn't Hugging Face".to_string())
    );
}

#[test]
fn lookalike_hosts_are_refused_on_every_hop() {
    for link in [
        "https://huggingface.co.evil.com/model.gguf",
        "https://evilhuggingface.co/model.gguf",
        "https://hf.co.evil.com/model.gguf",
        "https://evilhf.co/model.gguf",
    ] {
        assert!(check_download(link, Hop::First).is_err(), "{link}");
        assert!(check_download(link, Hop::Redirect).is_err(), "{link}");
    }
}

#[test]
fn a_link_naming_a_user_is_refused() {
    assert_eq!(
        check_download("https://huggingface.co@evil.com/model.gguf", Hop::First),
        Err("the link names a user or password".to_string())
    );
}

#[test]
fn a_link_naming_a_port_is_refused() {
    assert_eq!(
        check_download("https://huggingface.co:8443/model.gguf", Hop::First),
        Err("the link names a port".to_string())
    );
}

#[test]
fn ip_addresses_are_refused() {
    for link in [
        "https://13.32.0.1/model.gguf",
        "https://[2600:9000::1]/model.gguf",
    ] {
        assert_eq!(
            check_download(link, Hop::Redirect),
            Err("the link is to an IP address".to_string()),
            "{link}"
        );
    }
}

#[test]
fn hugging_face_cdn_hosts_are_allowed_only_on_redirect_hops() {
    for link in [
        "https://cas-bridge.xethub.hf.co/xet-bridge-us/model.gguf",
        "https://cdn-lfs.huggingface.co/repos/model.gguf",
    ] {
        assert_eq!(check_download(link, Hop::Redirect), Ok(()), "{link}");
        assert!(check_download(link, Hop::First).is_err(), "{link}");
    }
}

#[test]
fn a_redirect_to_a_relative_path_stays_on_the_same_host() {
    assert_eq!(
        follow_redirect(MODEL, "/api/resolve-cache/models/Qwen/model.gguf"),
        Ok("https://huggingface.co/api/resolve-cache/models/Qwen/model.gguf".to_string())
    );
}

#[test]
fn a_redirect_outside_hugging_face_is_refused() {
    assert_eq!(
        follow_redirect(MODEL, "https://evil.com/model.gguf"),
        Err("evil.com isn't Hugging Face".to_string())
    );
}
