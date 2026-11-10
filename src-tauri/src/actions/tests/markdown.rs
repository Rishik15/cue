//! File: actions/tests/markdown.rs
//! Purpose: Protect copy-as-Markdown: a web page on the clipboard becomes readable Markdown, and scripts or markup in it never become output.
//! Contents: tests of `to_markdown`: pages, scripts, truncation, attribute reading, no panics.

use crate::actions::to_markdown;
use proptest::prelude::*;

/// A small web page keeps its heading, link, emphasis and list, with entities decoded.
#[test]
fn a_copied_web_page_becomes_markdown_with_heading_link_emphasis_and_list() {
    let html = "<h1>Release &amp; notes</h1><p>Read the <a href=\"https://example.com/doc\">docs</a> and <b>test</b> it.</p>\
                <ul><li>one</li><li>two</li></ul><ol><li>first</li><li>second</li></ol>";
    let md = to_markdown(html);
    assert!(md.starts_with("# Release & notes"), "{md}");
    assert!(md.contains("[docs](https://example.com/doc)"), "{md}");
    assert!(md.contains("**test**"), "{md}");
    assert!(md.contains("- one\n- two"), "{md}");
    assert!(md.contains("1. first\n2. second"), "{md}");
}

/// Script and style bodies, comments and the title are dropped; a stray less-than sign is just text.
#[test]
fn scripts_styles_and_comments_never_reach_the_markdown() {
    let md = to_markdown(
        "<head><title>Page</title><style>p{color:red}</style></head><script>alert(1)</script><!-- note --><p>1 < 2 is true</p>",
    );
    assert_eq!(md, "1 < 2 is true");
}

/// A paste cut off inside a tag does not panic and keeps everything before the break.
#[test]
fn a_truncated_paste_keeps_the_text_before_the_broken_tag() {
    assert!(to_markdown("<p>kept text</p><a href=\"https://x").starts_with("kept text"));
}

/// The real href wins over look-alike attributes, a quoted alt keeps every word, and a `>` inside a quoted value does not end the tag.
#[test]
fn attributes_are_read_by_whole_name_and_quote() {
    assert_eq!(to_markdown("<a data-href=\"https://wrong.example\" href=\"https://right.example\">go</a>"), "[go](https://right.example)");
    assert_eq!(to_markdown("<a title='a > b' href='https://x.example/a?b=1&amp;c=2'>go</a>"), "[go](https://x.example/a?b=1&c=2)");
    assert_eq!(to_markdown("<img src=\"https://x.example/f.png\" alt=\"red fox\">"), "![red fox](https://x.example/f.png)");
    assert_eq!(to_markdown("<img alt=\"a > b\" src=x.png>after"), "![a > b](x.png)after");
    assert_eq!(to_markdown("<a href=https://x.example>go</a>"), "[go](https://x.example)");
}

proptest! {
    /// Any text, including broken and nested tags and odd quoting, converts without a panic.
    #[test]
    fn no_input_makes_the_converter_panic(html in "[<>a-z/='\" !&;#\\-]{0,120}|\\PC{0,80}") {
        let _ = to_markdown(&html);
    }
}
