//! File: snippets/tests/limits.rs
//! Purpose: Protect what a snippet may be: the limits on drafts that stop a bad page or file from storing nonsense, program matching and Markdown for formatted pasting.
//! Contents: tests of `check`, `TriggerSet`, `program_matches`, `program_entry` and the rich-text conversion.

use super::helpers::{draft, effective};
use crate::snippets::check::{check, clean, program_entry, program_matches, TriggerSet};
use crate::snippets::expand::richtext::{to_html, to_plain};
use crate::snippets::model::When;

/// A snippet needs a name and text, a bounded number of short triggers made of plain characters, and every refusal says so in words.
#[test]
fn a_draft_outside_the_limits_is_refused_with_a_message() {
    assert!(check(&draft("Sig", &[":sig"], "Best")).is_ok());
    assert_eq!(check(&draft("  ", &[":sig"], "Best")).err().as_deref(), Some("A snippet needs a name and some text."));
    assert_eq!(check(&draft("Sig", &[":sig"], "  \n ")).err().as_deref(), Some("A snippet needs a name and some text."));
    let nine: Vec<String> = (0..9).map(|i| format!(":t{i}")).collect();
    let nine: Vec<&str> = nine.iter().map(String::as_str).collect();
    assert!(check(&draft("Sig", &nine, "x")).is_err());
    for bad in ["x".repeat(41), "tab\there".to_string(), "e\u{301}".to_string(), "re:(oops".to_string(), "   ".to_string()] {
        assert!(check(&draft("Sig", &[bad.as_str()], "x")).is_err(), "{bad:?}");
    }
    assert!(check(&draft("Sig", &["good morning"], "x")).is_ok(), "a trigger may hold spaces");
}

/// Cleaning keeps what the user typed but lists a trigger once and one entry per program.
#[test]
fn cleaning_a_draft_trims_names_and_lists_each_trigger_and_program_once() {
    let mut messy = draft("  Sig  ", &[":sig", ":sig", "  ", " i "], "Best");
    messy.only_apps = vec!["Slack.exe".into(), "slack".into(), "Slack Desktop\tslack.exe".into()];
    let tidy = clean(&messy);
    assert_eq!((tidy.name.as_str(), tidy.triggers.as_slice()), ("Sig", [":sig".to_string(), " i ".to_string()].as_slice()));
    assert_eq!(tidy.only_apps, ["slack", "Slack Desktop\tslack"]);
}

/// Program names match whole words of the executable (`wezterm` finds `wezterm-gui`, `code` is not `codex`), and a picked entry must equal the executable.
#[test]
fn program_entries_match_whole_words_of_the_executable() {
    assert!(program_matches("wezterm", "wezterm-gui"));
    assert!(program_matches("Slack.exe", "slack"));
    assert!(!program_matches("code", "codex"));
    assert!(program_matches("WezTerm\twezterm-gui", "wezterm-gui"));
    assert!(!program_matches("WezTerm\twezterm-gui", "wezterm"));
    assert!(!program_matches("", "anything"));
    assert_eq!(program_entry("  Chrome.EXE "), "chrome");
    assert_eq!(program_entry("Chrome\tChrome.exe"), "Chrome\tchrome");
}

/// Markdown snippets paste with formatting in apps that take it and as readable plain text elsewhere; markup in the snippet is escaped and only safe links become links.
#[test]
fn markdown_becomes_safe_html_and_plain_text() {
    let md = "## Hello\nSee **this** and [site](https://example.com)\n- one\n- two\n\n<script>alert(1)</script> [bad](javascript:alert(1))";
    let html = to_html(md);
    assert!(
        html.starts_with(
            "<h2>Hello</h2><p>See <b>this</b> and <a href=\"https://example.com\">site</a></p><ul><li>one</li><li>two</li></ul>"
        ),
        "{html}"
    );
    assert!(html.contains("&lt;script&gt;") && !html.contains("<script>") && !html.contains("href=\"javascript"), "{html}");
    assert_eq!(to_plain("## Hello\nSee **this** and [site](https://example.com)"), "Hello\nSee this and site (https://example.com)");
}

/// The rule in its plainest form, for the oracle: the same text, or (both immediate) one that starts the other.
fn clash(a: (&str, When), b: (&str, When)) -> bool {
    let (x, y) = (a.0.to_lowercase(), b.0.to_lowercase());
    x == y || (a.1 == When::Immediate && b.1 == When::Immediate && (x.starts_with(&y) || y.starts_with(&x)))
}

proptest::proptest! {
    /// For any lists of triggers added in batches, `TriggerSet::try_add` agrees with the plain pairwise rule `clash` (equal, or an immediate one starting another), within a batch too, and a refused batch leaves nothing behind.
    #[test]
    fn the_trigger_set_agrees_with_the_pairwise_rule(batches in proptest::collection::vec(proptest::collection::vec("[:abA]{1,4}", 1..4), 1..8)) {
        let (mut set, mut accepted) = (TriggerSet::default(), Vec::new());
        for (id, batch) in batches.iter().enumerate() {
            let mine: Vec<_> = batch.iter().map(|t| effective(id as i64, t)).collect();
            let pairs = |a: &crate::snippets::policy::Effective, b: &crate::snippets::policy::Effective| clash((&a.trigger, a.when), (&b.trigger, b.when));
            let expected = mine.iter().enumerate().all(|(i, m)| !accepted.iter().chain(&mine[..i]).any(|o| pairs(m, o)));
            proptest::prop_assert_eq!(set.try_add(&mine).is_ok(), expected, "{:?} after {:?}", batch, accepted.iter().map(|e: &crate::snippets::policy::Effective| &e.trigger).collect::<Vec<_>>());
            if expected {
                accepted.extend(mine);
            }
        }
    }
}
