//! Purpose: Turns a snippet written in Markdown into formatted text for apps that take it (mail, documents, chat): HTML for the clipboard, and a plain version for apps that do not. Pure text in, text out.
//! Contents: to_html — headings, paragraphs (line breaks inside), bullet and numbered lists, **bold**, *italic*, `code` and [links](https://...); to_plain — the same text without the markers.
//! Only http, https and mailto links become links; everything else is escaped, so a snippet cannot inject markup. ponytail: no nesting of lists, no tables or images; extend when a snippet needs them.

use std::sync::LazyLock;

use regex::Regex;

/// Patterns for code, bold, italic and links, in the order they apply.
static INLINE: LazyLock<[Regex; 4]> = LazyLock::new(|| {
    ["`([^`]+)`", r"\*\*(.+?)\*\*", r"\*([^*]+)\*", r"\[([^\]]+)\]\((https?://[^)\s]+|mailto:[^)\s]+)\)"]
        .map(|p| Regex::new(p).expect("constant pattern"))
});

/// What each inline pattern becomes, as a replacement template.
const HTML_RULES: [&str; 4] = ["<code>$1</code>", "<b>$1</b>", "<i>$1</i>", r#"<a href="$2">$1</a>"#];
const PLAIN_RULES: [&str; 4] = ["$1", "$1", "$1", "$1 ($2)"];

fn inline(text: &str, rules: &[&str; 4]) -> String {
    INLINE.iter().zip(rules).fold(text.to_string(), |text, (pattern, with)| pattern.replace_all(&text, *with).into_owned())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `## Title` to (2, "Title").
fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|&c| c == '#').count();
    let rest = line[level..].strip_prefix(' ')?;
    (1..=6).contains(&level).then_some((level, rest.trim()))
}

/// `- item`, `* item`, `1. item` to the kind of list and the item's text.
fn list_item(line: &str) -> Option<(&'static str, &str)> {
    if let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|m| line.strip_prefix(m)) {
        return Some(("ul", rest.trim()));
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    line[digits..].strip_prefix(". ").filter(|_| digits > 0).map(|rest| ("ol", rest.trim()))
}

fn close(out: &mut String, open: &mut Option<&str>) {
    if let Some(tag) = open.take() {
        out.push_str(&format!("</{tag}>"));
    }
}

pub fn to_html(markdown: &str) -> String {
    let (mut out, mut open) = (String::new(), None::<&str>);
    for line in markdown.lines().map(str::trim) {
        if line.is_empty() {
            close(&mut out, &mut open);
        } else if let Some((level, text)) = heading(line) {
            close(&mut out, &mut open);
            out.push_str(&format!("<h{level}>{}</h{level}>", inline(&escape(text), &HTML_RULES)));
        } else if let Some((tag, text)) = list_item(line) {
            if open != Some(tag) {
                close(&mut out, &mut open);
                out.push_str(&format!("<{tag}>"));
                open = Some(tag);
            }
            out.push_str(&format!("<li>{}</li>", inline(&escape(text), &HTML_RULES)));
        } else {
            if open == Some("p") {
                out.push_str("<br>");
            } else {
                close(&mut out, &mut open);
                out.push_str("<p>");
                open = Some("p");
            }
            out.push_str(&inline(&escape(line), &HTML_RULES));
        }
    }
    close(&mut out, &mut open);
    out
}

pub fn to_plain(markdown: &str) -> String {
    let lines: Vec<String> =
        markdown.lines().map(|line| inline(heading(line.trim()).map_or(line, |(_, text)| text), &PLAIN_RULES)).collect();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_become_headings_paragraphs_and_lists() {
        assert_eq!(
            to_html("# Hi\nBest,\nMe\n\n- a\n- b\n1. c"),
            "<h1>Hi</h1><p>Best,<br>Me</p><ul><li>a</li><li>b</li></ul><ol><li>c</li></ol>"
        );
    }

    #[test]
    fn inline_marks_and_safe_links_are_converted_and_markup_is_escaped() {
        assert_eq!(
            to_html("**b** *i* `c` [x](https://a.b/?q=1&r=2)"),
            r#"<p><b>b</b> <i>i</i> <code>c</code> <a href="https://a.b/?q=1&amp;r=2">x</a></p>"#
        );
        assert_eq!(to_html("<script>[x](javascript:alert(1))"), "<p>&lt;script&gt;[x](javascript:alert(1))</p>");
    }

    #[test]
    fn the_plain_version_drops_the_markers_and_keeps_the_lines() {
        assert_eq!(to_plain("## T\n**b** and [x](https://a.b)\n- *i*"), "T\nb and x (https://a.b)\n- i");
    }
}
