//! Purpose: Remove tracking parameters from links inside text while keeping everything else, including unknown parameters.
//! Contents: clean — rewrite every link in a text; strip_tracking — one link; is_signed — links whose query is a signature, left untouched.

use super::extract::url_spans;

/// Parameters that only identify the click or campaign, never the page. Unknown parameters are always kept.
const TRACKING: [&str; 13] =
    ["fbclid", "gclid", "dclid", "msclkid", "yclid", "igshid", "mc_cid", "mc_eid", "_hsenc", "_hsmi", "mkt_tok", "twclid", "ttclid"];

pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for span in url_spans(text) {
        out.push_str(&text[at..span.start]);
        out.push_str(&strip_tracking(&text[span.clone()]));
        at = span.end;
    }
    out.push_str(&text[at..]);
    out
}

fn is_tracking(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_") || TRACKING.contains(&key.as_str())
}

/// A signed link (pre-signed download, expiring token) breaks if any parameter changes, so it is never rewritten.
fn is_signed(query: &str) -> bool {
    query.split('&').any(|p| {
        let key = p.split('=').next().unwrap_or("").to_ascii_lowercase();
        key.starts_with("x-amz-") || key.starts_with("x-goog-") || matches!(key.as_str(), "sig" | "signature" | "token" | "expires")
    })
}

fn strip_tracking(url: &str) -> String {
    let (rest, fragment) = url.split_once('#').map_or((url, None), |(a, b)| (a, Some(b)));
    let Some((base, query)) = rest.split_once('?') else { return url.to_string() };
    if is_signed(query) {
        return url.to_string();
    }
    let kept: Vec<&str> = query.split('&').filter(|p| !p.is_empty() && !is_tracking(p.split('=').next().unwrap_or(""))).collect();
    let mut out = base.to_string();
    if !kept.is_empty() {
        out = format!("{out}?{}", kept.join("&"));
    }
    if let Some(f) = fragment {
        out = format!("{out}#{f}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_tracking_parameters() {
        let text = "Read https://a.com/p?id=7&utm_source=x&fbclid=abc#top. Thanks";
        assert_eq!(clean(text), "Read https://a.com/p?id=7#top. Thanks");
        assert_eq!(clean("https://a.com/?utm_medium=m"), "https://a.com/");
    }

    #[test]
    fn leaves_signed_links_and_plain_text_alone() {
        let signed = "https://s3.example.com/f?X-Amz-Signature=1&utm_source=x";
        assert_eq!(clean(signed), signed);
        assert_eq!(clean("no links, just text"), "no links, just text");
    }
}
