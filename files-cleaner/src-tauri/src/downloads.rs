//! All of the app's network access. The app downloads only the Grouping
//! Models (ADR-0004), default or picked (ADR-0009), and only after the user
//! agrees; `models` asks for them through here, and nothing else in the app
//! uses the HTTP client (`tests/network.rs` fails if it does).
//!
//! Every request goes to Hugging Face over HTTPS: the first must be to
//! `huggingface.co`, and each redirect, followed here by hand, may go only
//! to it or to Hugging Face's own CDN hosts (`*.huggingface.co`,
//! `*.hf.co`). The check is `check_download` in the core's `download_hosts`.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use files_cleaner_core::{self as core, Hop};
use ureq::http::Response;
use ureq::Body;

/// A connection that takes this long to open, or then sends nothing for
/// this long, fails, so the user can resume.
const STALL_TIMEOUT: Duration = Duration::from_secs(30);
/// How many redirects one request may follow.
const MAX_REDIRECTS: u32 = 5;

/// How a download that didn't fail ended.
pub enum Fetched {
    /// The file is all there, the right size.
    Complete,
    /// `stop` said to stop; what's downloaded so far is kept.
    Stopped,
}

/// One request to send.
enum Request {
    /// Just the headers, to learn the file's size.
    Head,
    /// The file from byte `from` on.
    Get { from: u64 },
}

/// Downloads the file at `url`, `size` bytes, into `partial`, from where
/// that stopped: with a Range request, or from the start when the server
/// ignores it. A file that comes out the wrong size is deleted, so the next
/// try starts again. Checking its contents is the caller's. `stop` is asked
/// between reads, and `progress` called after each. The error says what
/// went wrong, for "Downloading … failed: {it}".
pub fn fetch(
    url: &str,
    size: u64,
    partial: &Path,
    stop: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(),
) -> Result<Fetched, String> {
    let failed = |e: &dyn std::fmt::Display| e.to_string();
    let mut offset = fs::metadata(partial).map(|m| m.len()).unwrap_or(0);
    if offset > size {
        fs::remove_file(partial).map_err(|e| failed(&e))?;
        offset = 0;
    }
    if offset < size {
        let response = request(Request::Get { from: offset }, url)?;
        let resumed = match response.status().as_u16() {
            206 => {
                let range = header(&response, "content-range");
                if !range.is_some_and(|range| range.starts_with(&format!("bytes {offset}-"))) {
                    return Err(failed(&"the server sent the wrong part of the file"));
                }
                true
            }
            // The server ignored the range: start again.
            200 => false,
            code => return Err(failed(&format!("the server answered {code}"))),
        };
        let mut out = OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(partial)
            .map_err(|e| failed(&e))?;
        let mut body = response.into_body().into_reader();
        let mut buffer = vec![0; 256 * 1024];
        loop {
            if stop() {
                return Ok(Fetched::Stopped);
            }
            let read = body.read(&mut buffer).map_err(|e| failed(&e))?;
            if read == 0 {
                break;
            }
            out.write_all(&buffer[..read]).map_err(|e| failed(&e))?;
            progress();
        }
        out.sync_all().map_err(|e| failed(&e))?;
    }
    if fs::metadata(partial).map_err(|e| failed(&e))?.len() != size {
        // A wrong file is never resumed: the next try starts again.
        let _ = fs::remove_file(partial);
        return Err(failed(&"the file came out the wrong size; try again"));
    }
    Ok(Fetched::Complete)
}

/// The size of the file at the Hugging Face download link `url`, or why it
/// can't be downloaded without a login.
pub fn remote_size(url: &str) -> Result<u64, String> {
    let response =
        request(Request::Head, url).map_err(|e| format!("Couldn't reach Hugging Face: {e}"))?;
    match response.status().as_u16() {
        200 => header(&response, "content-length")
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| "Hugging Face didn't say how big the file is.".to_string()),
        // Gated and private models, and repos that don't exist, all ask for a login.
        401 | 403 => Err("This model needs a Hugging Face login; pick a public one.".to_string()),
        404 => Err("Hugging Face has no file at that link.".to_string()),
        code => Err(format!("Hugging Face answered {code}.")),
    }
}

/// Sends `what` for `url`, following redirects by hand: `url` and each hop
/// are checked against the allowed hosts before anything is sent to them.
fn request(what: Request, url: &str) -> Result<Response<Body>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(STALL_TIMEOUT))
        .timeout_per_read(Some(STALL_TIMEOUT))
        .http_status_as_error(false)
        // Redirects are followed below, each hop checked.
        .max_redirects(0)
        .build()
        .into();
    core::check_download(url, Hop::First)?;
    let mut url = url.to_string();
    for _ in 0..=MAX_REDIRECTS {
        let response = match what {
            Request::Head => agent.head(&url).call(),
            Request::Get { from } => agent
                .get(&url)
                .header("Range", format!("bytes={from}-"))
                .call(),
        }
        .map_err(|e| e.to_string())?;
        if !response.status().is_redirection() {
            return Ok(response);
        }
        let location =
            header(&response, "location").ok_or("the server redirected without saying where")?;
        url = core::follow_redirect(&url, location)
            .map_err(|reason| format!("a redirect was refused: {reason}"))?;
    }
    Err("the server redirected too many times".to_string())
}

/// The value of the header `name` in `response`, if it's there and text.
fn header<'a>(response: &'a Response<Body>, name: &str) -> Option<&'a str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}
