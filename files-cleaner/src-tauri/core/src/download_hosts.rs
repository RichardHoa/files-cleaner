//! Every download request and redirect hop is checked here: the app promises to
//! reach Hugging Face only.

pub(crate) const DOWNLOAD_HOST: &str = "huggingface.co";

/// Hugging Face's CDN hosts change, so they are matched by suffix.
const REDIRECT_HOST_SUFFIXES: [&str; 2] = [".huggingface.co", ".hf.co"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hop {
    First,
    Redirect,
}

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

fn has_scheme(link: &str) -> bool {
    link.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// Refuses anything that could disguise where a link leads: user names, ports,
/// IP addresses.
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
