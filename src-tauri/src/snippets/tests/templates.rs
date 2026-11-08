//! File: snippets/tests/templates.rs
//! Purpose: Protect what a snippet's text becomes when it expands: dates and offsets, the clipboard, the caret, conditions, questions, nested snippets, literal braces, and that nothing untrusted can hide characters or loop.
//! Contents: scenario tests of `template::expand` and `template::arguments`.

use std::collections::HashMap;

use crate::platform::LocalTime;
use crate::snippets::expand::template::{arguments, expand, Argument, Context, Expanded};

/// Thursday 5 March 2026, 14:07:09.
const NOW: LocalTime = LocalTime { year: 2026, month: 3, day: 5, hour: 14, minute: 7, second: 9, weekday: 4 };

/// Expands `body` at `now` with a clipboard, a program and a table of other snippets. Inputs: the pieces. Returns the result.
fn run_at(now: LocalTime, body: &str, clipboard: Option<&str>, app: &str, others: &[(&str, &str)]) -> Expanded {
    let table: HashMap<String, String> = others.iter().map(|(n, b)| (n.to_string(), b.to_string())).collect();
    let lookup = move |name: &str| table.get(name).cloned();
    let ctx = Context {
        now,
        clipboard: clipboard.map(String::from),
        app: app.into(),
        counter: 7,
        lookup: &lookup,
        args: HashMap::from([("Name".to_string(), "Ann".to_string())]),
        captures: HashMap::from([("0".to_string(), "12px".to_string()), ("1".to_string(), "12".to_string())]),
    };
    expand(body, &ctx)
}

/// Expands at the fixed time with no clipboard, in notepad. Input: the text. Returns the expanded text.
fn text_of(body: &str) -> String {
    run_at(NOW, body, None, "notepad", &[]).text
}

/// Dates and times follow their layouts, and the clock can be moved by minutes, hours, days, weeks, months and years.
#[test]
fn dates_times_and_offsets_follow_their_layouts() {
    assert_eq!(text_of("{date} | {time} | {datetime} | {weekday}"), "5 Mar 2026 | 14:07 | 5 Mar 2026 14:07 | Thursday");
    assert_eq!(text_of("{date:YYYY-MM-DD} {time:h:mm A} {date:[Due] ddd D}"), "2026-03-05 2:07 PM Due Thu 5");
    assert_eq!(
        text_of("{date:YYYY-MM-DD | +3d} {date:YYYY-MM-DD | -1w} {date:YYYY-MM-DD | +1M} {date:YYYY | +1y}"),
        "2026-03-08 2026-02-26 2026-04-05 2027"
    );
    assert_eq!(text_of("{datetime:HH:mm | +10h} {datetime:dddd | +10h}"), "00:07 Friday");
}

/// Adding a month to the 31st lands on the last day of the shorter month, not the next one.
#[test]
fn a_month_offset_from_the_31st_stays_in_the_month() {
    let jan_31 = LocalTime { month: 1, day: 31, weekday: 6, ..NOW };
    assert_eq!(run_at(jan_31, "{date:YYYY-MM-DD | +1M}", None, "", &[]).text, "2026-02-28");
}

/// The clipboard is inserted with its modifiers, the caret placeholder says how far from the end the caret belongs, and hidden characters in the clipboard are stripped before anything is counted.
#[test]
fn clipboard_modifiers_the_caret_and_hidden_characters() {
    let out = run_at(NOW, "Hi {clipboard | trim | upper},{cursor} bye", Some("  sam  "), "", &[]);
    assert_eq!((out.text.as_str(), out.back), ("Hi SAM, bye", 4));
    let dirty = run_at(NOW, "[{clipboard}]{cursor}!", Some("ok\u{202E}evil\u{200B}"), "", &[]);
    assert_eq!((dirty.text.as_str(), dirty.back), ("[okevil]!", 1));
    assert_eq!(run_at(NOW, "{clipboard | title}", Some("hello wORLD"), "", &[]).text, "Hello World");
    assert_eq!(run_at(NOW, "a{cursor}b{cursor}c", None, "", &[]).back, 2, "the first caret placeholder wins");
}

/// Conditions pick text by program or clipboard; an unclosed block stays as typed.
#[test]
fn conditions_choose_by_program_and_clipboard() {
    let body = "{if app=slack, teams}Hey team{else}Hello{/if}!";
    assert_eq!(run_at(NOW, body, None, "slack", &[]).text, "Hey team!");
    assert_eq!(run_at(NOW, body, None, "word", &[]).text, "Hello!");
    assert_eq!(run_at(NOW, "{if app!=slack}elsewhere{/if}", None, "slack", &[]).text, "");
    assert_eq!(run_at(NOW, "{if clipboard}Pasted: {clipboard}{/if}", Some("  "), "", &[]).text, "");
    assert_eq!(run_at(NOW, "{if clipboard}Pasted: {clipboard}{/if}", Some("x"), "", &[]).text, "Pasted: x");
    assert_eq!(text_of("{if app=slack}never closed"), "{if app=slack}never closed");
}

/// Doubled braces are literal, unknown placeholders and stray braces stay exactly as typed, and an imported `{{env:...}}` types the placeholder instead of reading the environment.
#[test]
fn braces_unknown_placeholders_and_imported_env_stay_literal() {
    assert_eq!(text_of("{{date}} is {date:YYYY}"), "{date} is 2026");
    assert_eq!(text_of("{nonsense} and {oops and a { b"), "{nonsense} and {oops and a { b");
    assert_eq!(text_of("Token {{env:SECRET}}"), "Token {env:SECRET}");
}

/// Counters, answers and pattern matches fill in: a padded counter, a typed answer and the groups a regular-expression trigger captured.
#[test]
fn counters_answers_and_pattern_captures_fill_in() {
    assert_eq!(text_of("#{counter:3} {counter}"), "#007 7");
    assert_eq!(text_of("Dear {argument:Name}, {match:1} of {match}"), "Dear Ann, 12 of 12px");
}

/// The questions a text asks are listed once each, in order, with their choices; at most eight.
#[test]
fn arguments_are_listed_once_in_order_with_their_choices() {
    let found = arguments("Dear {argument:Name}, pick {argument:Size=S, M, L}; again {argument:Name} and {{argument:Literal}}");
    assert_eq!(
        found,
        [
            Argument { label: "Name".into(), choices: vec![] },
            Argument { label: "Size".into(), choices: vec!["S".into(), "M".into(), "L".into()] }
        ]
    );
    let many: String = (0..12).map(|i| format!("{{argument:Q{i}}}")).collect();
    assert_eq!(arguments(&many).len(), 8);
}

/// Snippets can include each other, but a snippet that includes itself stops, and a text full of includes is capped.
#[test]
fn nested_snippets_resolve_and_loops_and_floods_stop() {
    let others = [("sig", "Best, {snippet:name}"), ("name", "Rishik"), ("loop", "{snippet:loop}"), ("x", "x")];
    assert_eq!(run_at(NOW, "{snippet:sig}", None, "", &others).text, "Best, Rishik");
    assert_eq!(run_at(NOW, "{snippet:loop}", None, "", &others).text, "{snippet:loop}");
    let flood = "{snippet:x}".repeat(40);
    assert_eq!(run_at(NOW, &flood, None, "", &others).text, format!("{}{}", "x".repeat(32), "{snippet:x}".repeat(8)));
}

/// Random values stay inside what was asked for, and a UUID is a well-formed version 4 id.
#[test]
fn random_values_and_uuids_are_well_formed() {
    let roll: u32 = text_of("{random:1-6}").parse().expect("a number");
    assert!((1..=6).contains(&roll));
    assert!(["red", "green"].contains(&text_of("{random:red, green}").as_str()));
    let id = text_of("{uuid}");
    assert_eq!((id.len(), id.matches('-').count(), id.as_bytes()[14]), (36, 4, b'4'));
}
