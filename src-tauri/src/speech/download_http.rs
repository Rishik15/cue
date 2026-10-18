//! Purpose: The HTTP side of a model download: HTTPS-only requests whose redirects are followed by hand, so every hop is checked.
//! Contents: secure — the allowed address check; open — the response to read the file from, or None when the server rejects the resume range.

use std::time::Duration;

use super::Stop;

const MAX_HOPS: u8 = 5;

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

/// Asks for `url` from byte `have` on. Every hop must be HTTPS, so a redirect can never downgrade the download to plain HTTP.
/// `None`: the server says the range does not fit the file (HTTP 416), so the caller starts over.
pub(super) fn open(url: &str, have: u64) -> Result<Option<ureq::Response>, Stop> {
    let agent =
        ureq::AgentBuilder::new().redirects(0).timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(30)).build();
    let mut url = url.to_string();
    for _ in 0..=MAX_HOPS {
        if !secure(&url) {
            return Err(Stop::Failed("Models are only downloaded over HTTPS.".into()));
        }
        let mut request = agent.get(&url);
        if have > 0 {
            request = request.set("Range", &format!("bytes={have}-"));
        }
        let response = match request.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(416, _)) if have > 0 => return Ok(None),
            Err(ureq::Error::Status(code, _)) => {
                return Err(Stop::Failed(format!("The download is not available right now (error {code}). Try again later.")));
            }
            Err(_) => return Err(Stop::Failed("Download failed. Check your connection and try again.".into())),
        };
        if !(300..400).contains(&response.status()) {
            return Ok(Some(response));
        }
        url = next_url(&url, response.header("Location"))?;
    }
    Err(Stop::Failed("The download was redirected too many times.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_redirects_stay_on_the_host_and_absolute_ones_are_taken_as_is() {
        assert_eq!(next_url("https://a.com/x/y", Some("/z")).unwrap(), "https://a.com/z");
        assert_eq!(next_url("https://a.com", Some("/z")).unwrap(), "https://a.com/z");
        assert_eq!(next_url("https://a.com/x", Some("http://evil/z")).unwrap(), "http://evil/z");
        assert!(next_url("https://a.com", None).is_err());
        assert!(!secure("http://evil/z"));
    }
}
