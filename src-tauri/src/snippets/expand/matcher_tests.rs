//! Purpose: Tests of the trigger matcher: when a typed trigger counts, in which programs, and how patterns fire.
use super::*;
use crate::snippets::model::Mode;

fn rule(id: i64, trigger: &str, mode: Mode, match_case: bool) -> Effective {
    Effective {
        id,
        trigger: trigger.into(),
        mode,
        match_case,
        inside_word: false,
        propagate_case: !match_case,
        ending: String::new(),
        only_apps: Vec::new(),
        skip_apps: vec!["slack".into()],
    }
}

fn type_text(tracker: &mut Tracker, matcher: &Matcher, text: &str) -> Option<Hit> {
    text.chars().find_map(|c| tracker.feed(Typed::Char(c), matcher))
}

#[test]
fn a_symbol_trigger_fires_the_moment_it_is_typed_in_any_case() {
    let m = Matcher::new(&[rule(1, ":sig", Mode::Auto, false)]);
    let mut t = Tracker::default();
    let hit = type_text(&mut t, &m, "hi :SIG").unwrap();
    assert_eq!((hit.id, hit.erase, hit.retype, hit.typed.as_str(), hit.changed_case), (1, 4, None, ":SIG", true));
}

#[test]
fn a_letter_trigger_waits_for_a_space_and_gives_it_back() {
    let m = Matcher::new(&[rule(2, "brb", Mode::Auto, false)]);
    let mut t = Tracker::default();
    assert_eq!(type_text(&mut t, &m, "brb"), None);
    let hit = t.feed(Typed::Char(' '), &m).unwrap();
    assert_eq!((hit.id, hit.erase, hit.retype), (2, 4, Some(' ')));
    assert_eq!(type_text(&mut t, &m, "sbrb "), None, "inside a word it does not count");
}

#[test]
fn a_snippet_can_start_inside_a_word_and_end_on_its_own_characters() {
    let m = Matcher::new(&[Effective { inside_word: true, ending: "-".into(), ..rule(1, "ab", Mode::Delimiter, false) }]);
    let mut t = Tracker::default();
    type_text(&mut t, &m, "xab");
    assert_eq!(t.feed(Typed::Char(' '), &m), None, "a space is not one of its endings here");
    type_text(&mut t, &m, "xab");
    assert_eq!(t.feed(Typed::Char('-'), &m).map(|h| h.retype), Some(Some('-')));
}

#[test]
fn the_longest_trigger_wins_and_case_can_matter() {
    let m =
        Matcher::new(&[rule(1, "ba", Mode::Delimiter, false), rule(2, ":ba", Mode::Delimiter, true), rule(3, "x", Mode::Delimiter, true)]);
    let mut t = Tracker::default();
    type_text(&mut t, &m, ":ba");
    assert_eq!(t.feed(Typed::Char('.'), &m).map(|h| h.id), Some(2));
    type_text(&mut t, &m, ":bA");
    assert_eq!(t.feed(Typed::Char('.'), &m).map(|h| h.id), Some(1), "the match-case rule does not fit, the shorter one does");
}

#[test]
fn breaks_and_backspace_change_what_counts_and_enter_never_expands() {
    let m = Matcher::new(&[rule(1, ":ok", Mode::Auto, false), rule(2, "yo", Mode::Auto, false)]);
    let mut t = Tracker::default();
    assert_eq!(type_text(&mut t, &m, ":o"), None);
    t.feed(Typed::Reset, &m);
    assert_eq!(t.feed(Typed::Char('k'), &m), None);
    type_text(&mut t, &m, ":ox");
    t.feed(Typed::Backspace, &m);
    assert_eq!(t.feed(Typed::Char('k'), &m).map(|h| h.id), Some(1));
    type_text(&mut t, &m, "yo");
    assert_eq!(t.feed(Typed::Char('\n'), &m), None);
}

#[test]
fn programs_are_skipped_or_required_per_trigger() {
    let only = Effective { only_apps: vec!["code".into()], skip_apps: vec![], ..rule(2, ":b", Mode::Auto, false) };
    let m = Matcher::new(&[rule(1, ":sig", Mode::Auto, false), only]);
    assert!(!m.allows(0, "slack") && m.allows(0, "notepad"));
    assert!(m.allows(1, "code") && !m.allows(1, "notepad"));
}

#[test]
fn a_pattern_trigger_fires_at_its_end_and_reports_what_it_matched() {
    let m = Matcher::new(&[rule(1, r"re:(\d+)px", Mode::Auto, true), rule(2, "re:(", Mode::Auto, true)]);
    let mut t = Tracker::default();
    assert_eq!(type_text(&mut t, &m, "w 12p"), None);
    let hit = t.feed(Typed::Char('x'), &m).unwrap();
    assert_eq!((hit.id, hit.erase, hit.typed.as_str(), hit.changed_case), (1, 4, "12px", false));
    assert!(hit.captures.contains(&("1".into(), "12".into())));
}
