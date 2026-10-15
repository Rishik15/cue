//! Purpose: The clipboard's "HTML Format" (CF_HTML): reading the copied fragment out of it and wrapping a fragment into it. Pure bytes in and out, so it is tested without a clipboard.
//! Contents: fragment — the copied fragment of a CF_HTML block; build — a CF_HTML block for an HTML fragment.

const PREFIX: &str = "<html><body>\r\n<!--StartFragment-->";
const SUFFIX: &str = "<!--EndFragment-->\r\n</body></html>";

/// CF_HTML starts with a text header whose StartFragment and EndFragment are byte offsets into the whole block.
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

#[cfg(test)]
mod tests {
    use super::*;

    proptest::proptest! {
        #[test]
        fn a_clipboard_block_of_any_bytes_or_offsets_never_panics(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..300), start in 0usize..2000, end in 0usize..2000) {
            let _ = fragment(&bytes);
            let block = format!("Version:0.9\r\nStartFragment:{start:010}\r\nEndFragment:{end:010}\r\n<b>x</b>");
            let _ = fragment(block.as_bytes());
        }
    }

    #[test]
    fn the_html_fragment_is_cut_by_the_header_offsets() {
        let header = |start: usize, end: usize| format!("Version:0.9\r\nStartFragment:{start:010}\r\nEndFragment:{end:010}\r\n");
        let (before, inner) = ("<html><body><!--StartFragment-->", "<b>hi</b>");
        let start = header(0, 0).len() + before.len();
        let block = format!("{}{before}{inner}<!--EndFragment--></body></html>", header(start, start + inner.len()));
        assert_eq!(fragment(block.as_bytes()).as_deref(), Some(inner));
        assert_eq!(fragment(b"no header"), None);
    }

    #[test]
    fn a_built_block_reads_back_to_its_fragment_with_exact_offsets() {
        let html = "<b>Grüße</b> — ok";
        let block = build(html);
        assert_eq!(fragment(&block).as_deref(), Some(html));
        let text = String::from_utf8_lossy(&block).to_string();
        let end_html: usize = text.split("EndHTML:").nth(1).unwrap()[..10].parse().unwrap();
        assert_eq!(end_html, block.len() - 1, "EndHTML stops before the closing NUL");
    }
}
