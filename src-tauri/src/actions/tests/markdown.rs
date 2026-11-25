//! File: actions/tests/markdown.rs
//! Purpose: Protect copy-as-Markdown: a web page on the clipboard becomes readable Markdown, and scripts or markup in it never become output.
//! Contents: tests of `to_markdown`: pages, scripts, truncation, attribute reading, no panics, the output cap.

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

/// Nesting depth of `n` bulleted lists around one `x`.
fn nested(n: usize) -> String {
    format!("{}<li>x</li>{}", "<ul><li>".repeat(n), "</li></ul>".repeat(n))
}

proptest! {
    /// Deep lists indent at most six levels, and any depth up to 20,000 gives a bounded result that is either empty or keeps the text.
    #[test]
    fn nested_lists_stay_bounded(depth in 1usize..20_000) {
        let md = to_markdown(&nested(depth));
        prop_assert!(md.len() < 4 << 20);
        prop_assert!(md.is_empty() || md.contains('x'));
        prop_assert!(md.lines().all(|l| l.len() - l.trim_start().len() <= 12));
    }
}

/// A 90 KB nested-list page stays small, and depth 10 indents 12 spaces at the most.
#[test]
fn a_90_kb_nested_list_is_bounded() {
    let html = nested(90_000 / 20);
    let md = to_markdown(&html);
    assert!(md.len() < 1 << 20, "{}", md.len());
    assert!(md.contains('x'));
    assert!(to_markdown(&nested(10)).lines().all(|l| l.len() - l.trim_start().len() <= 12));
}

/// Output past the cap is no result at all, not a half one.
#[test]
fn output_past_the_cap_is_empty() {
    assert_eq!(to_markdown(&"<p>word</p>".repeat(1_500_000)), "");
}

/// Text dropped at the cap stays an overflow even when a closing list trims the buffer back under it: the result is empty, never missing `LOST`.
#[test]
fn text_dropped_at_the_cap_is_never_hidden_by_a_trim() {
    let html = format!("{}{}<li><br><br><em>LOST</em>{}", "<ul>".repeat(7), "<li>x</li>".repeat(262_143), "</ul>".repeat(7));
    assert!(to_markdown(&html).is_empty());
}

/// A megabyte of bare ampersands decodes in linear time (the old scan to the next semicolon from every one took minutes) and comes back unchanged; real references still decode.
#[test]
fn a_megabyte_of_ampersands_is_linear() {
    let text = "&".repeat(1 << 20);
    assert_eq!(to_markdown(&text), text);
    assert_eq!(to_markdown("&amp;&#x41;&nbsp;&bogus;"), "&A &bogus;");
}
