//! Tests of the placeholders: dates, clipboard, cursor, counters, snippets inside snippets, arguments, conditions and the escapes.
use super::*;

fn lookup(name: &str) -> Option<String> {
    match name {
        "sig" => Some("Best,\n{snippet:name}".into()),
        "name" => Some("Rishik".into()),
        "loop" => Some("a{snippet:loop}".into()),
        "wide" => Some("{snippet:wide}".repeat(1400)),
        _ => None,
    }
}

fn ctx(clipboard: Option<&str>) -> Context<'static> {
    Context {
        now: LocalTime { year: 2026, month: 6, day: 1, hour: 15, minute: 4, second: 9, weekday: 1 },
        clipboard: clipboard.map(String::from),
        app: "slack".into(),
        counter: 7,
        lookup: &lookup,
        args: HashMap::from([("Name".to_string(), "Ada".to_string())]),
        captures: HashMap::from([("0".to_string(), "12px".to_string()), ("1".to_string(), "12".to_string())]),
    }
}

fn text(body: &str, clipboard: Option<&str>) -> String {
    expand(body, &ctx(clipboard)).text
}

#[test]
fn dates_and_times_use_their_default_or_a_given_layout() {
    assert_eq!(text("{date} {time} {weekday}", None), "1 Jun 2026 15:04 Monday");
    assert_eq!(text("{date:YYYY-MM-DD} {time:h:mm A} {date:dddd, D MMMM [at] HH}", None), "2026-06-01 3:04 PM Monday, 1 June at 15");
}

#[test]
fn the_clipboard_comes_through_modifiers_and_is_empty_when_there_is_none() {
    assert_eq!(text("[{clipboard | trim | upper}]", Some("  hi there ")), "[HI THERE]");
    assert_eq!(text("[{clipboard}]", None), "[]");
    assert_eq!(text("{clipboard | title}", Some("hello WORLD")), "Hello World");
}

#[test]
fn the_cursor_is_counted_from_the_end_and_only_the_first_counts() {
    assert_eq!(expand("Dear {cursor},\nBest", &ctx(None)), Expanded { text: "Dear ,\nBest".into(), back: 6 });
    assert_eq!(expand("a{cursor}b{cursor}c", &ctx(None)).back, 2);
    assert_eq!(expand("none", &ctx(None)).back, 0);
}

#[test]
fn unknown_names_and_stray_braces_stay_as_typed_and_doubled_ones_are_literal() {
    assert_eq!(text("{nope} {date | nope} { x {{date}} }", None), "{nope} {date | nope} { x {date} }");
    assert_eq!(text("fn() { return 1; }", None), "fn() { return 1; }");
}

#[test]
fn only_a_body_that_asks_for_the_clipboard_needs_it() {
    assert!(uses_clipboard("x {clipboard | trim}") && !uses_clipboard("x {date}"));
}

#[test]
fn dates_move_by_offsets_in_any_order() {
    assert_eq!(text("{date:YYYY-MM-DD | +3d}", None), "2026-06-04");
    assert_eq!(text("{datetime:D/M HH:mm | -1d | +2h}", None), "31/5 17:04");
    assert_eq!(text("{weekday | +1w}", None), "Monday");
}

#[test]
fn counter_app_and_env_fill_in_and_the_counter_pads() {
    assert_eq!(text("#{counter} #{counter:4} {app}", None), "#7 #0007 slack");
    std::env::set_var("CUE_TEMPLATE_TEST", "x");
    assert_eq!(text("{env:CUE_TEMPLATE_TEST}{env:CUE_NOT_SET}.", None), "x.");
}

#[test]
fn random_and_uuid_have_the_right_shape() {
    assert!(["a", "b"].contains(&text("{random:a, b}", None).as_str()));
    let n: i64 = text("{random:3-5}", None).parse().unwrap();
    assert!((3..=5).contains(&n));
    assert_eq!(text("{random:5-3} {random:}", None), "{random:5-3} {random:}");
    let id = text("{uuid}", None);
    assert!(id.len() == 36 && id.as_bytes()[14] == b'4' && id.matches('-').count() == 4);
}

#[test]
fn a_body_full_of_references_stops_after_a_fixed_number_of_lookups() {
    assert!(text(&"{snippet:wide}".repeat(1400), None).len() < 1_000_000);
}

#[test]
fn other_snippets_nest_and_a_loop_stops() {
    assert_eq!(text("{snippet:sig}", None), "Best,\nRishik");
    assert_eq!(text("{snippet:missing}", None), "{snippet:missing}");
    assert_eq!(text("{snippet:loop}", None), "aaa{snippet:loop}");
}

#[test]
fn arguments_are_listed_once_with_their_choices_and_filled_from_the_answers() {
    let body = "Hi {argument:Name}, {argument: Tone = warm, formal | upper} {argument:Name} {{argument:Skipped}} {argument:}";
    let list = arguments(body);
    assert_eq!(list.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(), ["Name", "Tone"]);
    assert_eq!(list[1].choices, ["warm", "formal"]);
    assert_eq!(text("Hi {argument:Name}, {argument:Other=a,b}.", None), "Hi Ada, .");
}

#[test]
fn conditions_keep_the_branch_that_holds() {
    assert_eq!(text("{if app=slack, teams}hey{else}dear{/if} you", None), "hey you");
    assert_eq!(text("{if app!=slack}a{else}b{/if}", None), "b");
    assert_eq!(text("{if app=word}x{/if}.", None), ".");
    assert_eq!(text("{if clipboard}[{clipboard}]{else}empty{/if}", Some("hi")), "[hi]");
    assert_eq!(text("{if clipboard}yes{else}empty{/if}", Some("  ")), "empty");
    assert_eq!(text("{if app=slack}never closed", None), "{if app=slack}never closed");
}

#[test]
fn match_gives_the_whole_match_or_a_group() {
    assert_eq!(text("{match} is {match:1} wide, {match:9}.", None), "12px is 12 wide, .");
}

#[test]
fn a_doubled_brace_before_if_is_literal_and_an_unclosed_block_keeps_the_text_before_it() {
    assert_eq!(text("{{if app=slack}yes{/if}", None), "{if app=slack}yes{/if}");
    assert_eq!(text("before {if app=slack}never closed", None), "before {if app=slack}never closed");
    assert_eq!(text("a {if app=slack}b{/if} c {if app=x}", None), "a b c {if app=x}");
}

#[test]
fn hidden_characters_from_the_clipboard_are_stripped_before_the_caret_is_counted() {
    let out = expand("{clipboard}{cursor}!", &ctx(Some("a\u{202E}b\u{200B}")));
    assert_eq!((out.text.as_str(), out.back), ("ab!", 1));
}
