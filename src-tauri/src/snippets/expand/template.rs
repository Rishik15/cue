//! Purpose: Turns a snippet's text into what is typed: placeholders become the date, the clipboard, a counter and so on, and mark where the caret should end up. Pure logic: the time, the clipboard and other snippets come in as values.
//! Contents: Context / Expanded — what goes in and out; expand — the text; uses_clipboard — whether reading the clipboard is needed at all; placeholder and its helpers — one `{...}` to text.
//! Placeholders: {match} {match:1} (what a `re:` trigger matched), {argument:Label} {argument:Label=a,b} (asked for before typing), {date} {time} {datetime} {weekday} {clipboard} {cursor} {counter} {uuid} {random:a,b,c} {random:1-6} {env:NAME} {app} {snippet:name}, with a layout after a colon ({date:YYYY-MM-DD}, {time:h:mm A}) and modifiers
//! after bars: text ({clipboard | trim | upper}) and offsets for dates ({date | +3d}, {datetime | -2h}; units m h d w M y).
//! Conditions: {if app=slack, teams}...{else}...{/if}, {if app!=slack}, {if clipboard} (one level, no nesting).
//! Written {{ and }} give a literal brace; anything between braces that is not a placeholder stays as typed.

use std::collections::HashMap;

use regex::Regex;
use serde::Serialize;

use super::timefmt::{format_time, parse_offset, shift};
use super::values::{modify, random, uuid};
use crate::platform::LocalTime;

pub struct Context<'a> {
    pub now: LocalTime,
    pub clipboard: Option<String>,
    /// The program that has the focus.
    pub app: String,
    /// What `{counter}` gives: the number of times this snippet has been used, plus one.
    pub counter: i64,
    /// The text of another snippet by name or trigger, for `{snippet:name}`.
    pub lookup: &'a dyn Fn(&str) -> Option<String>,
    /// What the user typed for each `{argument:Label}`, by label.
    pub args: HashMap<String, String>,
    /// What a pattern trigger matched, by group number (`0` is the whole match) or name.
    pub captures: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Expanded {
    pub text: String,
    /// How many characters from the end the caret belongs, when the text has a {cursor}; 0 otherwise.
    pub back: usize,
}

/// The longest a placeholder may be, so a stray brace does not swallow a page of text.
const MAX_PLACEHOLDER: usize = 80;
/// How deep `{snippet:...}` may go; a snippet that includes itself stops here and stays as typed.
const MAX_DEPTH: usize = 3;

/// A value `{argument:Label}` asks for, with the allowed answers when it was written `{argument:Label=a, b}`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Argument {
    pub label: String,
    pub choices: Vec<String>,
}

/// The most questions one snippet may ask.
const MAX_ARGUMENTS: usize = 8;

/// `Label=a, b` to the label and its text of choices.
fn split_spec(spec: &str) -> (&str, &str) {
    spec.split_once('=').map_or((spec.trim(), ""), |(label, choices)| (label.trim(), choices))
}

/// The questions a text asks, in order of first appearance, each label once. (Arguments inside a nested `{snippet:...}` are not asked.)
pub fn arguments(body: &str) -> Vec<Argument> {
    let found = Regex::new(r"(?i)(?:^|[^{])\{\s*argument\s*:([^{}|]*)").expect("constant pattern");
    let mut out: Vec<Argument> = Vec::new();
    for caps in found.captures_iter(body) {
        let (label, choices) = split_spec(&caps[1]);
        if label.is_empty() || out.len() >= MAX_ARGUMENTS || out.iter().any(|a| a.label == label) {
            continue;
        }
        let choices = choices.split(',').map(str::trim).filter(|c| !c.is_empty()).map(String::from).collect();
        out.push(Argument { label: label.into(), choices });
    }
    out
}

pub fn uses_clipboard(body: &str) -> bool {
    body.contains("{clipboard") || body.contains("{if clipboard")
}

/// Keeps the branch of each `{if cond}yes{else}no{/if}` that holds (`{else}` is optional). One level only: blocks do not nest; a block that is not closed stays as typed.
fn resolve_ifs(body: &str, ctx: &Context) -> String {
    let (mut out, mut rest) = (String::new(), body);
    while let Some(start) = rest.find("{if ") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        let Some((cond, tail)) = after.split_once('}') else { break };
        let Some((block, next)) = tail.split_once("{/if}") else { break };
        let (yes, no) = block.split_once("{else}").unwrap_or((block, ""));
        out.push_str(if holds(cond, ctx) { yes } else { no });
        rest = next;
    }
    out.push_str(rest);
    out
}

/// `clipboard` (it holds text), `app=slack, teams` (the program is one of these) or `app!=slack`.
fn holds(cond: &str, ctx: &Context) -> bool {
    let cond = cond.trim().to_lowercase();
    let is_in = |list: &str| list.split(',').any(|a| a.trim().trim_end_matches(".exe") == ctx.app);
    match (cond.split_once("!="), cond.split_once('=')) {
        (Some(("app", list)), _) => !is_in(list),
        (_, Some(("app", list))) => is_in(list),
        _ => cond == "clipboard" && ctx.clipboard.as_deref().is_some_and(|c| !c.trim().is_empty()),
    }
}

pub fn expand(body: &str, ctx: &Context) -> Expanded {
    expand_at(body, ctx, 0)
}

fn expand_at(body: &str, ctx: &Context, depth: usize) -> Expanded {
    let (mut out, mut cursor) = (String::new(), None::<usize>);
    let chars: Vec<char> = resolve_ifs(body, ctx).chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if (c == '{' && next == Some('{')) || (c == '}' && next == Some('}')) {
            out.push(c);
            i += 2;
            continue;
        }
        let end = (c == '{').then(|| chars[i + 1..].iter().take(MAX_PLACEHOLDER).position(|&x| x == '}' || x == '{')).flatten();
        match end.filter(|&e| chars[i + 1 + e] == '}') {
            Some(e) => {
                let inner: String = chars[i + 1..i + 1 + e].iter().collect();
                match placeholder(&inner, ctx, depth) {
                    Some(Piece::Text(text)) => out.push_str(&text),
                    Some(Piece::Cursor) => {
                        cursor.get_or_insert(out.chars().count());
                    }
                    None => out.push_str(&format!("{{{inner}}}")),
                }
                i += e + 2;
            }
            None => {
                out.push(c);
                i += 1;
            }
        }
    }
    let total = out.chars().count();
    Expanded { text: out, back: cursor.map_or(0, |at| total - at) }
}

enum Piece {
    Text(String),
    Cursor,
}

/// `name`, `name:argument`, either followed by `| modifier` items.
fn placeholder(inner: &str, ctx: &Context, depth: usize) -> Option<Piece> {
    let mut parts = inner.split('|').map(str::trim);
    let head = parts.next()?;
    let (name, arg) = head.split_once(':').map_or((head, None), |(n, l)| (n.trim(), Some(l.trim())));
    let (offsets, modifiers): (Vec<_>, Vec<_>) = parts.partition(|m| parse_offset(m).is_some());
    let now = offsets.iter().filter_map(|m| parse_offset(m)).fold(ctx.now, |t, (n, unit)| shift(&t, n, unit));
    let text = match name.to_lowercase().as_str() {
        "cursor" => return Some(Piece::Cursor),
        "date" => format_time(arg.unwrap_or("D MMM YYYY"), &now),
        "time" => format_time(arg.unwrap_or("HH:mm"), &now),
        "datetime" => format_time(arg.unwrap_or("D MMM YYYY HH:mm"), &now),
        "weekday" => format_time(arg.unwrap_or("dddd"), &now),
        "clipboard" => ctx.clipboard.clone().unwrap_or_default(),
        "counter" => format!("{:0width$}", ctx.counter, width = arg.and_then(|a| a.parse().ok()).unwrap_or(0).min(20)),
        "uuid" => uuid(),
        "random" => random(arg?)?,
        "env" => std::env::var(arg?).unwrap_or_default(),
        "app" => ctx.app.clone(),
        "match" => ctx.captures.get(arg.unwrap_or("0")).cloned().unwrap_or_default(),
        "argument" => ctx.args.get(split_spec(arg?).0).cloned().unwrap_or_default(),
        "snippet" if depth < MAX_DEPTH => expand_at(&(ctx.lookup)(arg?)?, ctx, depth + 1).text,
        _ => return None,
    };
    modifiers.into_iter().try_fold(text, |text, modifier| modify(&text, modifier)).map(Piece::Text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<String> {
        match name {
            "sig" => Some("Best,\n{snippet:name}".into()),
            "name" => Some("Rishik".into()),
            "loop" => Some("a{snippet:loop}".into()),
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
}
