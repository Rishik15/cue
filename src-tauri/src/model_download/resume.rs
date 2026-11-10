//! File: model_download/resume.rs
//! Purpose: The pure rules that decide whether a server's answer to a resume request can be trusted: the Content-Range must cover exactly the rest of the file, the body must not be re-encoded, only a strong ETag may guard a resume, and Retry-After says how long to wait.
//! Contents: strong — is an ETag usable as an If-Range validator; identity — is the body unencoded; check_range — the 206 answer fits the file; retry_after — seconds to wait from a header.

use std::time::Duration;

/// The longest pause a server may ask for with Retry-After; anything longer is cut to this.
const MAX_WAIT: Duration = Duration::from_secs(120);

/// True when `etag` is a strong validator: a quoted tag with no `W/` weak prefix. Only a strong ETag may go in If-Range.
pub(super) fn strong(etag: &str) -> bool {
    let tag = etag.trim();
    tag.len() >= 2 && tag.starts_with('"') && tag.ends_with('"')
}

/// True when the Content-Encoding header says the bytes are not re-encoded (missing or `identity`); an encoded body cannot match the pinned hash of the file.
pub(super) fn identity(encoding: Option<&str>) -> bool {
    encoding.is_none_or(|e| e.trim().is_empty() || e.trim().eq_ignore_ascii_case("identity"))
}

/// Parses `bytes <start>-<end>/<total>`; returns start, end and the total (None when the server writes `*`).
fn parse_range(header: &str) -> Option<(u64, u64, Option<u64>)> {
    let rest = header.trim().strip_prefix("bytes ")?;
    let (span, total) = rest.split_once('/')?;
    let (start, end) = span.split_once('-')?;
    let total = if total.trim() == "*" { None } else { Some(total.trim().parse().ok()?) };
    Some((start.trim().parse().ok()?, end.trim().parse().ok()?, total))
}

/// Checks a 206 answer: it must start at byte `have`, run to the last byte of the `size`-byte file and, when the server states a total, agree with `size`. Returns the reason when it does not.
pub(super) fn check_range(content_range: Option<&str>, have: u64, size: u64) -> Result<(), &'static str> {
    let Some((start, end, total)) = content_range.and_then(parse_range) else {
        return Err("The download server sent no valid range. Try again.");
    };
    if start != have {
        return Err("The download server resumed at the wrong place. Try again.");
    }
    if end + 1 != size || total.is_some_and(|t| t != size) {
        return Err("The download server's file is not the one expected. Try again later.");
    }
    Ok(())
}

/// Reads a Retry-After header given in seconds (the date form is ignored); returns the wait, at most two minutes.
pub(super) fn retry_after(value: Option<&str>) -> Option<Duration> {
    let secs: u64 = value?.trim().parse().ok()?;
    Some(Duration::from_secs(secs).min(MAX_WAIT))
}
