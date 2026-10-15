//! Purpose: Tests of the trigger matcher: when a typed trigger counts, in which programs, how spaces in a trigger behave, and how patterns fire.
use super::*;

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

fn matcher(triggers: &[&str]) -> Matcher {
    Matcher::new(&triggers.iter().enumerate().map(|(i, t)| rule(i as i64, t, Mode::Auto, false)).collect::<Vec<_>>())
}

fn type_text(tracker: &mut Tracker, matcher: &Matcher, text: &str) -> Option<Hit> {
    text.chars().find_map(|c| tracker.feed(Typed::Char(c), matcher))
}

#[test]
fn a_symbol_trigger_fires_the_moment_it_is_typed_in_any_case() {
    let m = Matcher::new(&[rule(1, ":sig", Mode::Auto, false)]);
    let hit = type_text(&mut Tracker::default(), &m, "hi :SIG").unwrap();
    assert_eq!((hit.id, hit.erase, hit.retype.as_str(), hit.typed.as_str(), hit.changed_case), (1, 4, "", ":SIG", true));
}

#[test]
fn a_letter_trigger_waits_for_a_space_and_gives_it_back() {
    let m = Matcher::new(&[rule(2, "brb", Mode::Auto, false)]);
    let mut t = Tracker::default();
    assert_eq!(type_text(&mut t, &m, "brb"), None);
    let hit = t.feed(Typed::Char(' '), &m).unwrap();
    assert_eq!((hit.id, hit.erase, hit.retype.as_str()), (2, 4, " "));
    assert_eq!(type_text(&mut t, &m, "sbrb "), None, "inside a word it does not count");
}

#[test]
fn a_one_letter_trigger_is_a_whole_word_and_any_ending_gives_the_word_back() {
    let m = matcher(&["i"]);
    let mut t = Tracker::default();
    assert_eq!(type_text(&mut t, &m, "hi "), None, "the i of hi is inside a word");
    let hit = type_text(&mut t, &m, "so i ").unwrap();
    assert_eq!((hit.erase, hit.retype.as_str(), hit.typed.as_str()), (2, " ", "i"));
    let hit = type_text(&mut t, &m, "i'").unwrap();
    assert_eq!((hit.erase, hit.retype.as_str()), (2, "'"), "i'm becomes I'm: the apostrophe ends the word");
    assert_eq!(type_text(&mut t, &m, "if "), None);
}

#[test]
fn spaces_at_the_ends_of_a_trigger_are_required_but_kept() {
    // `i ` fires on its own space, ` i ` also needs the space before it, ` i` waits for what ends the word.
    let m = matcher(&["i "]);
    let hit = type_text(&mut Tracker::default(), &m, "so i ").unwrap();
    assert_eq!((hit.erase, hit.retype.as_str(), hit.typed.as_str()), (2, " ", "i"));
    let m = matcher(&[" i "]);
    let mut t = Tracker::default();
    assert_eq!(type_text(&mut t, &m, "hi "), None, "no space before the i");
    let hit = type_text(&mut t, &m, "so i ").unwrap();
    assert_eq!((hit.erase, hit.retype.as_str(), hit.typed.as_str()), (2, " ", "i"), "the leading space stays");
    let m = matcher(&[" i"]);
    let mut t = Tracker::default();
    type_text(&mut t, &m, "so i");
    assert_eq!(t.feed(Typed::Char('.'), &m).map(|h| (h.erase, h.retype)), Some((2, ".".to_string())));
}

#[test]
fn several_triggers_for_one_snippet_give_the_same_edit() {
    let m = matcher(&["i", "i ", " i "]);
    for typed in ["so i ", "i ", "(i "] {
        let hit = type_text(&mut Tracker::default(), &m, typed).unwrap_or_else(|| panic!("{typed:?} fires"));
        assert_eq!((hit.erase, hit.retype.as_str()), (2, " "), "{typed:?}");
    }
}

#[test]
fn a_trigger_may_contain_spaces_inside() {
    let m = matcher(&["good morning"]);
    let mut t = Tracker::default();
    type_text(&mut t, &m, "say good morning");
    let hit = t.feed(Typed::Char('!'), &m).unwrap();
    assert_eq!((hit.erase, hit.retype.as_str(), hit.typed.as_str()), (13, "!", "good morning"));
    let hit = type_text(&mut t, &m, "very good morning ").unwrap();
    assert_eq!((hit.erase, hit.retype.as_str()), (13, " "), "a space ends it too, and inner spaces survive in the typed text");
}

#[test]
fn typed_text_is_dropped_at_a_space_unless_some_trigger_contains_one() {
    let mut t = Tracker::default();
    type_text(&mut t, &matcher(&[":a"]), "ab cd");
    assert_eq!(t.typed.iter().collect::<String>(), "cd", "only what follows the space is held");
    let mut t = Tracker::default();
    type_text(&mut t, &matcher(&["a b"]), "x y");
    assert_eq!(t.typed.iter().collect::<String>(), "x y");
}

#[test]
fn a_snippet_can_start_inside_a_word_and_end_on_its_own_characters() {
    let m = Matcher::new(&[Effective { inside_word: true, ending: "-".into(), ..rule(1, "ab", Mode::Delimiter, false) }]);
    let mut t = Tracker::default();
    type_text(&mut t, &m, "xab");
    assert_eq!(t.feed(Typed::Char(' '), &m), None, "a space is not one of its endings here");
    type_text(&mut t, &m, "xab");
    assert_eq!(t.feed(Typed::Char('-'), &m).map(|h| h.retype), Some("-".to_string()));
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
    let m = matcher(&[":ok", "yo"]);
    let mut t = Tracker::default();
    assert_eq!(type_text(&mut t, &m, ":o"), None);
    t.feed(Typed::Reset, &m);
    assert_eq!(t.feed(Typed::Char('k'), &m), None);
    type_text(&mut t, &m, ":ox");
    t.feed(Typed::Backspace, &m);
    assert_eq!(t.feed(Typed::Char('k'), &m).map(|h| h.id), Some(0));
    type_text(&mut t, &m, "yo");
    assert_eq!(t.feed(Typed::Char('\n'), &m), None);
}

#[test]
fn programs_are_skipped_or_required_per_trigger() {
    let only = Effective { only_apps: vec!["code".into()], skip_apps: vec![], ..rule(2, ":b", Mode::Auto, false) };
    let m = Matcher::new(&[rule(1, ":sig", Mode::Auto, false), only]);
    let names = |list: &[&str]| list.iter().map(|n| n.to_string()).collect::<Vec<_>>();
    assert!(!m.allows(0, &names(&["slack"])) && m.allows(0, &names(&["notepad"])));
    assert!(m.allows(1, &names(&["code"])) && !m.allows(1, &names(&["notepad"])));
    assert!(m.allows(1, &names(&["code - insiders", "visual studio code - insiders"])), "the program's own name counts too");
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
