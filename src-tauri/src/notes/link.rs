//! File: notes/link.rs
//! Purpose: Opens a link from a note in the default browser. The text comes from the page, so only a plain web address is accepted.
//! Contents: open — checks the address and hands it to the browser; valid — the check, apart from the OS call so it is testable.

const LONGEST: usize = 2048;

/// Whether `url` is a plain web address Cue may open: http or https, at most `LONGEST` bytes, no whitespace or control characters. Input: the address. Returns the verdict.
fn valid(url: &str) -> bool {
    let web = url.starts_with("https://") || url.starts_with("http://");
    web && url.len() <= LONGEST && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Opens a web address from a note in the default browser. Input: the address from the page. Returns Err with a user-readable text when it is refused or cannot be opened.
pub fn open(url: &str) -> Result<(), String> {
    if !valid(url) {
        return Err("Only web addresses open from a note.".into());
    }
    crate::platform::open_url(url)
}
