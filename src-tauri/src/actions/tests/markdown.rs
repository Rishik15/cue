//! File: actions/tests/markdown.rs
//! Purpose: Protect copy-as-Markdown: a web page on the clipboard becomes readable Markdown, and scripts or markup in it never become output.
//! Contents: tests of `to_markdown`.

use crate::actions::to_markdown;

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
