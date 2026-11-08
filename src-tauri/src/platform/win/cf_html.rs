//! File: cf_html.rs
//! Purpose: The clipboard's "HTML Format" (CF_HTML): reading the copied fragment out of it and wrapping a fragment into it. Pure bytes in and out, so it needs no clipboard.
//! Contents: fragment — the copied fragment of a CF_HTML block; build — a CF_HTML block for an HTML fragment.

/// The markup before the fragment in a CF_HTML block.
const PREFIX: &str = "<html><body>\r\n<!--StartFragment-->";
/// The markup after the fragment in a CF_HTML block.
const SUFFIX: &str = "<!--EndFragment-->\r\n</body></html>";

/// The copied fragment of a CF_HTML block, trimmed; None when the header is missing or malformed or the fragment is empty. CF_HTML starts with a text header whose StartFragment and EndFragment are byte offsets into the whole block.
pub fn fragment(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(400)]);
    let number = |key: &str| -> Option<usize> {
        let at = head.find(key)? + key.len();
        head[at..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
    };
    let (start, end) = (number("StartFragment:")?, number("EndFragment:")?);
    let fragment = bytes.get(start..end.min(bytes.len()))?;
    Some(String::from_utf8_lossy(fragment).trim().to_string()).filter(|f| !f.is_empty())
}

/// A CF_HTML block: a header with byte offsets (fixed-width numbers, so the header's own length is known first), the fragment between its markers, and a closing NUL.
pub fn build(fragment: &str) -> Vec<u8> {
    let header = |html: (usize, usize), frag: (usize, usize)| {
        format!(
            "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n",
            html.0, html.1, frag.0, frag.1
        )
    };
    let start_html = header((0, 0), (0, 0)).len();
    let start_fragment = start_html + PREFIX.len();
    let end_fragment = start_fragment + fragment.len();
    let end_html = end_fragment + SUFFIX.len();
    let mut out = header((start_html, end_html), (start_fragment, end_fragment)).into_bytes();
    out.extend_from_slice(PREFIX.as_bytes());
    out.extend_from_slice(fragment.as_bytes());
    out.extend_from_slice(SUFFIX.as_bytes());
    out.push(0);
    out
}
