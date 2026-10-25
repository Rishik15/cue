//! Purpose: Turns the HTML a program puts on the clipboard (a web page, a document, a mail) into Markdown, with no model and no parser dependency.
//! Contents: to_markdown — HTML to Markdown; Out — the writer that tracks lists, links, quotes and preformatted text; character references are decoded by `entities`.
//! shortcut: common tags only (headings, emphasis, links, lists, code, quotes, images, simple tables as rows); nested tables and Word-specific markup come out as plain text. Swap in an HTML parser crate if real documents need more.

use super::entities::decode;

#[derive(Default)]
struct Out {
    buf: String,
    /// Inside style, script or head: nothing is written.
    skip: usize,
    pre: bool,
    quote: usize,
    /// Open lists, innermost last: the next number of an ordered list, None for a bulleted one.
    lists: Vec<Option<u32>>,
    /// One entry per open link: its address, so the closing tag knows whether to write one.
    links: Vec<Option<String>>,
}

pub fn to_markdown(html: &str) -> String {
    let mut out = Out::default();
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.text(&rest[..lt]);
        rest = &rest[lt..];
        if !rest[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!') {
            out.text("<"); // a lone less-than sign is text
            rest = &rest[1..];
            continue;
        }
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.split_once("-->").map_or("", |(_, tail)| tail);
            continue;
        }
        let Some((tag, tail)) = rest[1..].split_once('>') else { break };
        out.tag(tag);
        rest = tail;
    }
    out.text(rest);
    out.finish()
}

fn attr(tag: &str, name: &str) -> Option<String> {
    // Case-insensitive on ASCII only, searched in the original: a full Unicode lowercase can change byte lengths and shift every position after it.
    let key = format!("{name}=");
    let at = tag.as_bytes().windows(key.len()).position(|w| w.eq_ignore_ascii_case(key.as_bytes()))? + key.len();
    let value = tag.get(at..)?.trim_start_matches(['"', '\'']);
    let end = value.find(['"', '\'', ' ', '>']).unwrap_or(value.len());
    Some(decode(&value[..end]))
}

impl Out {
    fn write(&mut self, s: &str) {
        if self.skip == 0 {
            self.buf.push_str(s);
        }
    }

    fn newline(&mut self) {
        if !self.buf.is_empty() && !self.buf.ends_with('\n') {
            self.buf.push('\n');
        }
    }

    fn paragraph(&mut self) {
        let kept = self.buf.trim_end_matches([' ', '\n']).len();
        self.buf.truncate(kept);
        if !self.buf.is_empty() {
            self.buf.push_str("\n\n");
        }
    }

    fn text(&mut self, raw: &str) {
        if self.skip > 0 || raw.is_empty() {
            return;
        }
        let text = decode(raw);
        if self.pre {
            return self.buf.push_str(&text);
        }
        for (i, word) in text.split_whitespace().enumerate() {
            if self.buf.ends_with('\n') && self.quote > 0 {
                self.buf.push_str("> ");
            }
            let spaced = i > 0 || text.starts_with(char::is_whitespace);
            if spaced && !self.buf.is_empty() && !self.buf.ends_with([' ', '\n', '[', '(']) {
                self.buf.push(' ');
            }
            self.buf.push_str(word);
        }
        if text.ends_with(char::is_whitespace) && !self.buf.ends_with([' ', '\n']) && !self.buf.is_empty() {
            self.buf.push(' ');
        }
    }

    fn tag(&mut self, raw: &str) {
        let closing = raw.starts_with('/');
        let name: String = raw.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
        if matches!(name.as_str(), "style" | "script" | "head" | "title") {
            self.skip = if closing { self.skip.saturating_sub(1) } else { self.skip + 1 };
            return;
        }
        if self.skip > 0 {
            return;
        }
        match name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.heading(closing, name[1..].parse().unwrap_or(1)),
            "p" | "div" | "section" | "article" | "table" => self.paragraph(),
            "br" => self.write("\n"),
            "hr" => self.write("\n\n---\n\n"),
            "b" | "strong" => self.write("**"),
            "i" | "em" => self.write("*"),
            "code" if !self.pre => self.write("`"),
            "pre" => self.preformatted(closing),
            "a" => self.link(closing, raw),
            "ul" | "ol" => self.list(closing, name == "ol"),
            "li" if !closing => self.item(),
            "li" | "tr" => self.newline(),
            "td" | "th" if closing => self.write(" | "),
            "blockquote" => self.quoted(closing),
            "img" => self.image(raw),
            _ => {}
        }
    }

    fn heading(&mut self, closing: bool, level: usize) {
        self.paragraph();
        if !closing {
            self.write(&format!("{} ", "#".repeat(level)));
        }
    }

    fn preformatted(&mut self, closing: bool) {
        if closing {
            self.pre = false;
            self.newline();
            return self.write("```\n\n");
        }
        self.paragraph();
        self.write("```\n");
        self.pre = true;
    }

    fn link(&mut self, closing: bool, raw: &str) {
        if !closing {
            let href = attr(raw, "href").filter(|h| !h.is_empty());
            self.write(if href.is_some() { "[" } else { "" });
            return self.links.push(href);
        }
        if let Some(Some(href)) = self.links.pop() {
            self.write(&format!("]({href})"));
        }
    }

    fn list(&mut self, closing: bool, ordered: bool) {
        if closing {
            self.lists.pop();
            return if self.lists.is_empty() { self.paragraph() } else { self.newline() };
        }
        if self.lists.is_empty() {
            self.paragraph();
        } else {
            self.newline();
        }
        self.lists.push(ordered.then_some(1));
    }

    fn item(&mut self) {
        self.newline();
        let depth = self.lists.len().saturating_sub(1);
        let marker = match self.lists.last_mut() {
            Some(Some(n)) => {
                *n += 1;
                format!("{}. ", *n - 1)
            }
            _ => "- ".into(),
        };
        self.write(&format!("{}{marker}", "  ".repeat(depth)));
    }

    fn quoted(&mut self, closing: bool) {
        self.paragraph();
        self.quote = if closing { self.quote.saturating_sub(1) } else { self.quote + 1 };
        if !closing {
            self.write("> ");
        }
    }

    fn image(&mut self, raw: &str) {
        if let Some(src) = attr(raw, "src") {
            self.write(&format!("![{}]({src})", attr(raw, "alt").unwrap_or_default()));
        }
    }

    fn finish(mut self) -> String {
        let lines: Vec<&str> = self.buf.lines().map(str::trim_end).collect();
        self.buf = lines.join("\n");
        while self.buf.contains("\n\n\n") {
            self.buf = self.buf.replace("\n\n\n", "\n\n");
        }
        self.buf.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_emphasis_links_and_paragraphs() {
        let html =
            "<h1>Title</h1><p>Some <b>bold</b> and <i>italic</i> with a <a href=\"https://a.com/x?y=1&amp;z=2\">link</a>.</p><p>Next</p>";
        assert_eq!(to_markdown(html), "# Title\n\nSome **bold** and *italic* with a [link](https://a.com/x?y=1&z=2).\n\nNext");
    }

    #[test]
    fn lists_nest_and_numbers_count() {
        assert_eq!(to_markdown("<ul><li>a</li><li>b<ul><li>c</li></ul></li></ul>"), "- a\n- b\n  - c");
        assert_eq!(to_markdown("<ol><li>one</li><li>two</li></ol>"), "1. one\n2. two");
    }

    #[test]
    fn code_quotes_images_and_rules() {
        assert_eq!(to_markdown("<p>Use <code>cargo test</code></p>"), "Use `cargo test`");
        assert_eq!(to_markdown("<pre>let a = 1;\nlet b = 2;</pre>"), "```\nlet a = 1;\nlet b = 2;\n```");
        assert_eq!(to_markdown("<blockquote>quoted</blockquote>"), "> quoted");
        assert_eq!(to_markdown("<img src=\"a.png\" alt=\"A\">"), "![A](a.png)");
    }

    #[test]
    fn styles_scripts_comments_and_entities_do_not_leak() {
        let html = "<head><style>p{color:red}</style></head><!-- note --><script>alert(1)</script><p>a&nbsp;&lt;b&gt; &#8212; &#x41; &unknown;</p>";
        assert_eq!(to_markdown(html), "a <b> \u{2014} A &unknown;");
    }

    #[test]
    fn attributes_are_found_after_characters_whose_lowercase_has_another_length() {
        assert_eq!(to_markdown("<a title=\"\u{130}\u{130}\u{130}\" HREF=x>link</a>"), "[link](x)");
        assert_eq!(to_markdown("<img alt=\"\u{1E9E}\" SRC=\"a.png\">"), "![\u{1E9E}](a.png)");
    }

    #[test]
    fn plain_text_passes_through_and_broken_markup_does_not_panic() {
        assert_eq!(to_markdown("just  text\nhere"), "just text here");
        assert_eq!(to_markdown("a < b and <b>unclosed"), "a < b and **unclosed");
    }
}
