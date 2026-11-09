//! File: model_download/http.rs
//! Purpose: The HTTP side of a model download: HTTPS-only requests whose redirects are followed by hand, so every hop is checked, and cut off at once when the download is cancelled.
//! Contents: HttpNet — the production `Net`; secure — the allowed address check; open / hops — the response to read the file from (redirects by hand), or None when the server rejects the resume range; header — one response header as text; next_url — where a redirect points.

use std::time::Duration;

use super::ctl::Ctl;
use super::interrupt::{interruptible, PumpedBody};
use super::net::{Fetched, Net};
use super::transfer::Stop;

const MAX_HOPS: u8 = 5;

type Response = ureq::http::Response<ureq::Body>;

/// A connection that delivers nothing for this long is cut and the download resumes from the partial file. A cancel does not wait for it: blocking calls run through `interrupt`.
/// shortcut: a cancelled call is abandoned on its helper thread, which ends at its own deadline (resolve, connect, idle read) and may hold its socket until then; upgrade: a connector that closes the socket handle.
const IDLE: Duration = if cfg!(test) { Duration::from_secs(1) } else { Duration::from_secs(10) };

/// A response header as text; None when it is missing or not text.
fn header<'a>(response: &'a Response, name: &str) -> Option<&'a str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}

/// Downloads go over HTTPS only. The tests talk to a loopback server over plain HTTP, so the test build also accepts that address (and nothing else).
pub(super) fn secure(url: &str) -> bool {
    url.starts_with("https://") || (cfg!(test) && url.starts_with("http://127.0.0.1"))
}

/// Where a redirect points: an absolute address, a scheme-relative one (`//host/path`), a path on the same host, or a path relative to the current one.
fn next_url(base: &str, location: Option<&str>) -> Result<String, Stop> {
    let location = location.ok_or_else(|| Stop::Failed("The download server sent a broken redirect.".into()))?;
    if location.starts_with("https://") || location.starts_with("http://") {
        return Ok(location.to_string());
    }
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    if let Some(rest) = location.strip_prefix("//") {
        return Ok(format!("{}//{rest}", &base[..scheme_end.saturating_sub(2)]));
    }
    let path_start = base[scheme_end..].find('/').map_or(base.len(), |i| scheme_end + i);
    if location.starts_with('/') {
        return Ok(format!("{}{location}", &base[..path_start]));
    }
    let directory_end = base[path_start..].rfind('/').map_or(base.len(), |i| path_start + i + 1);
    let directory = if path_start == base.len() { format!("{base}/") } else { base[..directory_end].to_string() };
    Ok(format!("{directory}{location}"))
}

/// The production network.
pub struct HttpNet;

impl Net for HttpNet {
    /// Opens `url` over HTTPS from byte `have` on; see `open`.
    fn open(&self, url: &str, have: u64, ctl: &Ctl) -> Result<Option<Fetched>, Stop> {
        Ok(open(url, have, ctl)?.map(|(response, closer)| Fetched {
            status: response.status().as_u16(),
            content_range: header(&response, "Content-Range").map(str::to_string),
            body: Box::new(PumpedBody::new(response.into_body().into_reader(), ctl, vec![closer])),
        }))
    }
}

/// Asks for `url` from byte `have` on. Cancelling `ctl` shuts the connection and abandons a blocked call. Every hop must be HTTPS, so a redirect can never downgrade the download to plain HTTP.
/// `None`: the server says the range does not fit the file (HTTP 416), so the caller starts over. On a response the id of the socket closer is returned: it stays registered for the life of the body.
fn open(url: &str, have: u64, ctl: &Ctl) -> Result<Option<(Response, u64)>, Stop> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .timeout_resolve(Some(Duration::from_secs(10)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(IDLE))
        .build()
        .into();
    let closer = agent.clone();
    let id = ctl.register(Box::new(move || closer.shutdown()));
    let answer = if id == 0 { Err(Stop::Cancelled) } else { hops(&agent, url, have, ctl) };
    match answer {
        Ok(Some(response)) => Ok(Some((response, id))),
        other => {
            ctl.unregister(id);
            other.map(|_| None)
        }
    }
}

/// Follows the redirects of `url` by hand and returns the response to read, or None for HTTP 416.
fn hops(agent: &ureq::Agent, url: &str, have: u64, ctl: &Ctl) -> Result<Option<Response>, Stop> {
    let mut url = url.to_string();
    for _ in 0..=MAX_HOPS {
        if !secure(&url) {
            return Err(Stop::Failed("Models are only downloaded over HTTPS.".into()));
        }
        let (caller, address) = (agent.clone(), url.clone());
        let called = interruptible(ctl, move || {
            let mut request = caller.get(&address);
            if have > 0 {
                request = request.header("Range", format!("bytes={have}-"));
            }
            request.call()
        })?;
        let response = called.map_err(|_| {
            if ctl.is_cancelled() {
                Stop::Cancelled
            } else {
                Stop::Offline("Download failed. Check your connection and try again.".into())
            }
        })?;
        match response.status().as_u16() {
            416 if have > 0 => return Ok(None),
            code if code >= 500 => return Err(Stop::Offline(format!("The download server is busy (error {code}). Try again later."))),
            code if code >= 400 => {
                return Err(Stop::Failed(format!("The download is not available right now (error {code}). Try again later.")))
            }
            300..=399 => url = next_url(&url, header(&response, "Location"))?,
            _ => return Ok(Some(response)),
        }
    }
    Err(Stop::Failed("The download was redirected too many times.".into()))
}
