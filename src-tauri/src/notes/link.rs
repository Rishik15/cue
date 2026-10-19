//! Purpose: Opens a link from a note in the default browser. The text comes from the page, so only a plain web address is accepted.
//! Contents: open — checks the address and hands it to the browser; valid — the check, apart from the OS call so it is testable.

const LONGEST: usize = 2048;

fn valid(url: &str) -> bool {
    let web = url.starts_with("https://") || url.starts_with("http://");
    web && url.len() <= LONGEST && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

pub fn open(url: &str) -> Result<(), String> {
    if !valid(url) {
        return Err("Only web addresses open from a note.".into());
    }
    crate::platform::open_url(url)
}

#[cfg(test)]
mod tests {
    use super::{valid, LONGEST};

    #[test]
    fn only_plain_web_addresses() {
        assert!(valid("https://example.com/a?b=1"));
        assert!(valid("http://example.com"));
        assert!(!valid("file:///c:/windows"));
        assert!(!valid("javascript:alert(1)"));
        assert!(!valid("https://a.com/ b"));
        assert!(!valid("https://a.com/\n"));
        assert!(!valid(&format!("https://a.com/{}", "x".repeat(LONGEST))));
    }
}
