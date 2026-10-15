//! Purpose: Decodes HTML character references (&amp;, &nbsp;, &#39;, &#x2014;) for the Markdown converter.
//! Contents: decode — a text with its references replaced; reference — one reference name to a character.

/// The character references in a text, decoded.
pub fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        match tail.find(';').filter(|&i| i <= 10).and_then(|i| reference(&tail[1..i]).map(|c| (c, i))) {
            Some((c, i)) => {
                out.push(c);
                rest = &tail[i + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn reference(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}
