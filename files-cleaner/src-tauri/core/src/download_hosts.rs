//! Where the app may download from: Hugging Face only, over HTTPS. The
//! Grouping Models, default or picked (ADR 0009), are the only thing the app
//! downloads, and the app's downloads module checks every request and every
//! redirect hop here before making it.

/// The only host a download may start at.
pub(crate) const DOWNLOAD_HOST: &str = "huggingface.co";

/// What a redirect hop's host may end with, at a label boundary, besides
/// being `DOWNLOAD_HOST`: Hugging Face's own CDN hosts, which change.
const REDIRECT_HOST_SUFFIXES: [&str; 2] = [".huggingface.co", ".hf.co"];

/// Which request of a download a link is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hop {
    /// The link the download starts at.
    First,
    /// Where a redirect sent it.
    Redirect,
}

/// `Ok` when the app may request `url` as `hop`, or why not.
pub fn check_download(url: &str, hop: Hop) -> Result<(), String> {
    let host = host(url)?;
    let allowed = host == DOWNLOAD_HOST
        || (hop == Hop::Redirect
            && REDIRECT_HOST_SUFFIXES.iter().any(|suffix| {
                host.strip_suffix(suffix)
                    .is_some_and(|label| !label.is_empty() && !label.ends_with('.'))
            }));
    match allowed {
        true => Ok(()),
        false => Err(format!("{host} isn't Hugging Face")),
    }
}

/// Where a redirect from `from` to `location` (its `Location` header) goes,
/// once checked as a redirect hop, or why it can't be followed.
pub fn follow_redirect(from: &str, location: &str) -> Result<String, String> {
    let location = location.trim();
    let next = if location.starts_with("//") {
        format!("https:{location}")
    } else if location.starts_with('/') {
        format!("https://{}{location}", host(from)?)
    } else if has_scheme(location) {
        location.to_string()
    } else {
        return Err("the server redirected the download somewhere unclear".to_string());
    };
    check_download(&next, Hop::Redirect)?;
    Ok(next)
}

/// Whether `link` starts with a URL scheme, such as `https:`.
fn has_scheme(link: &str) -> bool {
    link.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// The lower-case host of the HTTPS link `url`, refusing a link with a
/// user name or password, a port or an IP address, or a host that isn't a
/// plain name.
fn host(url: &str) -> Result<String, String> {
    let rest = url
        .get(.."https://".len())
        .filter(|scheme| scheme.eq_ignore_ascii_case("https://"))
        .map(|_| &url["https://".len()..])
        .ok_or("the link isn't HTTPS")?;
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    if authority.contains('@') {
        return Err("the link names a user or password".to_string());
    }
    if authority.starts_with('[') {
        return Err("the link is to an IP address".to_string());
    }
    if authority.contains(':') {
        return Err("the link names a port".to_string());
    }
    let host = authority.to_ascii_lowercase();
    let labels: Vec<&str> = host.split('.').collect();
    let plain = labels.iter().all(|label| {
        !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    });
    if !plain {
        return Err(format!("{authority} isn't a plain host name"));
    }
    // A last label of digits only makes the host an IPv4 address, as browsers read it.
    if labels
        .last()
        .is_some_and(|last| last.chars().all(|c| c.is_ascii_digit()))
    {
        return Err("the link is to an IP address".to_string());
    }
    Ok(host)
}
