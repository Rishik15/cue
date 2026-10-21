//! Purpose: The HTTP side of a model download: HTTPS-only requests whose redirects are followed by hand, so every hop is checked.
//! Contents: secure — the allowed address check; open — the response to read the file from, or None when the server rejects the resume range; header — one response header as text.

use std::time::Duration;

use super::Stop;

const MAX_HOPS: u8 = 5;

pub(super) type Response = ureq::http::Response<ureq::Body>;

/// Slowest speed a download may have before it is given up (and resumed by the next attempt), and the time added for slow starts.
const MIN_BYTES_PER_SEC: u64 = 50_000;
const BODY_SLACK: Duration = Duration::from_secs(120);

/// How long the body may take. The HTTP client has no timeout per read, only one for the whole body, so a stalled connection is cut off by a time that grows with the bytes still to come.
fn body_budget(remaining: u64) -> Duration {
    BODY_SLACK + Duration::from_secs(remaining / MIN_BYTES_PER_SEC)
}

/// A response header as text; None when it is missing or not text.
pub(super) fn header<'a>(response: &'a Response, name: &str) -> Option<&'a str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}

/// Downloads go over HTTPS only. The tests talk to a loopback server over plain HTTP, so the test build also accepts that address (and nothing else).
pub(super) fn secure(url: &str) -> bool {
    url.starts_with("https://") || (cfg!(test) && url.starts_with("http://127.0.0.1"))
}

/// Where a redirect points: an absolute address, or a path on the same host.
fn next_url(base: &str, location: Option<&str>) -> Result<String, Stop> {
    let location = location.ok_or_else(|| Stop::Failed("The download server sent a broken redirect.".into()))?;
    if !location.starts_with('/') {
        return Ok(location.to_string());
    }
    let host = base.find("://").map_or(0, |i| i + 3);
    let end = base[host..].find('/').map_or(base.len(), |i| host + i);
    Ok(format!("{}{location}", &base[..end]))
}

/// Asks for `url` from byte `have` on, with `remaining` bytes still to come (it sets the time the body may take). Every hop must be HTTPS, so a redirect can never downgrade the download to plain HTTP.
/// `None`: the server says the range does not fit the file (HTTP 416), so the caller starts over.
pub(super) fn open(url: &str, have: u64, remaining: u64) -> Result<Option<Response>, Stop> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(body_budget(remaining)))
        .build()
        .into();
    let mut url = url.to_string();
    for _ in 0..=MAX_HOPS {
        if !secure(&url) {
            return Err(Stop::Failed("Models are only downloaded over HTTPS.".into()));
        }
        let mut request = agent.get(&url);
        if have > 0 {
            request = request.header("Range", format!("bytes={have}-"));
        }
        let response = request.call().map_err(|_| Stop::Offline("Download failed. Check your connection and try again.".into()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_budget_grows_with_the_bytes_still_to_come() {
        assert_eq!(body_budget(0), Duration::from_secs(120));
        assert_eq!(body_budget(500_000_000), Duration::from_secs(120 + 10_000));
    }

    #[test]
    fn relative_redirects_stay_on_the_host_and_absolute_ones_are_taken_as_is() {
        assert_eq!(next_url("https://a.com/x/y", Some("/z")).unwrap(), "https://a.com/z");
        assert_eq!(next_url("https://a.com", Some("/z")).unwrap(), "https://a.com/z");
        assert_eq!(next_url("https://a.com/x", Some("http://evil/z")).unwrap(), "http://evil/z");
        assert!(next_url("https://a.com", None).is_err());
        assert!(!secure("http://evil/z"));
    }
}
