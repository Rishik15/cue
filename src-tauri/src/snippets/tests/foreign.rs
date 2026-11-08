//! File: snippets/tests/foreign.rs
//! Purpose: Protect importing from other tools (Espanso, TextExpander, AutoHotkey): their formats read into snippets, and their own placeholders can never run as Cue placeholders.
//! Contents: tests of `foreign::parse`.

use crate::snippets::foreign::parse;
use crate::snippets::model::Mode;

/// An Espanso file with plain, multi-trigger and block-scalar matches imports each with the right triggers and text, and braces become literal so another tool's text cannot run Cue placeholders.
#[test]
fn an_espanso_file_imports_all_three_shapes_with_literal_braces() {
    let yaml = "matches:\n  - trigger: \":sig\"\n    replace: \"Best,\\nRishik\"\n  - triggers: [\":a\", \":b\"]\n    replace: |\n      line one\n      line two\n  - trigger: \":env\"\n    replace: \"{env:SECRET} {date}\"\n";
    let (drafts, refused) = parse("yml", yaml).expect("parses");
    assert_eq!(refused, 0);
    assert_eq!(drafts.len(), 3);
    assert_eq!((drafts[0].0.triggers.as_slice(), drafts[0].0.body.as_str()), ([":sig".to_string()].as_slice(), "Best,\nRishik"));
    assert_eq!((drafts[1].0.triggers.len(), drafts[1].0.body.as_str()), (2, "line one\nline two"));
    assert_eq!(drafts[2].0.body, "{{env:SECRET}} {{date}}");
}

/// TextExpander CSV: the header row is skipped and quoted cells may hold commas and line breaks.
#[test]
fn a_textexpander_csv_skips_the_header_and_reads_quoted_cells() {
    let csv = "abbreviation,content\n;sig,\"Best,\nRishik\"\n;addr,12 Main St\n";
    let (drafts, refused) = parse("csv", csv).expect("parses");
    assert_eq!(refused, 0);
    let seen: Vec<(&str, &str)> = drafts.iter().map(|(d, _)| (d.triggers[0].as_str(), d.body.as_str())).collect();
    assert_eq!(seen, [(";sig", "Best,\nRishik"), (";addr", "12 Main St")]);
}

/// AutoHotkey hotstrings keep their options: `*` fires at once, `?` works inside words, `C` matches case, and backtick escapes are read.
#[test]
fn autohotkey_hotstrings_keep_their_options_and_escapes() {
    let ahk = "::btw::by the way\n:*:addr::12 Main`nSpringfield\n:C?:BTW::Big Tech Weekly\n; a comment line\n";
    let (drafts, refused) = parse("ahk", ahk).expect("parses");
    assert_eq!((drafts.len(), refused), (3, 0));
    assert_eq!((drafts[0].0.mode, drafts[0].0.body.as_str()), (Mode::Auto, "by the way"));
    assert_eq!((drafts[1].0.mode, drafts[1].0.body.as_str()), (Mode::Immediate, "12 Main\nSpringfield"));
    assert!(drafts[2].0.match_case && drafts[2].0.inside_word);
}

/// Entries that break the limits are counted as refused, and other extensions fall back to Cue's own format.
#[test]
fn invalid_foreign_entries_are_counted_and_unknown_extensions_use_cue_format() {
    let (kept, refused) = parse("csv", "abbreviation,content\n;ok,fine\n;empty,   \n").expect("parses");
    assert_eq!((kept.len(), refused), (1, 1));
    let (cue, _) = parse("json", r#"{"version":3,"snippets":[{"name":"A","triggers":[":a"],"body":"x"}]}"#).expect("json");
    assert_eq!(cue.len(), 1);
}
