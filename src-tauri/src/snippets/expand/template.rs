//! Purpose: Turns a snippet's text into what is typed: placeholders become the date, the clipboard, a counter and so on, and mark where the caret should end up. Pure logic: the time, the clipboard and other snippets come in as values.
//! Contents: Context / Expanded — what goes in and out; expand — the text; uses_clipboard — whether reading the clipboard is needed at all; placeholder and its helpers — one `{...}` to text.
//! Placeholders: {match} {match:1} (what a `re:` trigger matched), {argument:Label} {argument:Label=a,b} (asked for before typing), {date} {time} {datetime} {weekday} {clipboard} {cursor} {counter} {uuid} {random:a,b,c} {random:1-6} {env:NAME} {app} {snippet:name}, with a layout after a colon ({date:YYYY-MM-DD}, {time:h:mm A}) and modifiers
//! after bars: text ({clipboard | trim | upper}) and offsets for dates ({date | +3d}, {datetime | -2h}; units m h d w M y).
//! Conditions: {if app=slack, teams}...{else}...{/if}, {if app!=slack}, {if clipboard} (one level, no nesting).
//! Written {{ and }} give a literal brace; anything between braces that is not a placeholder stays as typed.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::timefmt::{format_time, parse_offset, shift};
use super::values::{modify, random, uuid};
use crate::platform::LocalTime;
use crate::sanitize::strip_hidden;

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
/// How many `{snippet:...}` lookups one expansion may make in all; depth alone still allows N^3 growth from a body full of references.
const MAX_LOOKUPS: usize = 32;

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
    static FOUND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:^|[^{])\{\s*argument\s*:([^{}|]*)").expect("constant pattern"));
    let mut out: Vec<Argument> = Vec::new();
    for caps in FOUND.captures_iter(body) {
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

/// Keeps the branch of each `{if cond}yes{else}no{/if}` that holds (`{else}` is optional). One level only: blocks do not nest; a block that is not closed stays as typed. `{{if` is a literal brace, not a condition.
fn resolve_ifs(body: &str, ctx: &Context) -> String {
    let (mut out, mut at) = (String::new(), 0);
    while let Some(found) = body[at..].find("{if ") {
        let start = at + found;
        if body[..start].chars().rev().take_while(|&c| c == '{').count() % 2 == 1 {
            out.push_str(&body[at..start + 1]);
            at = start + 1;
            continue;
        }
        out.push_str(&body[at..start]);
        at = start; // from here on the text stays as typed unless the block turns out whole
        let Some((cond, tail)) = body[start + 4..].split_once('}') else { break };
        let Some((block, next)) = tail.split_once("{/if}") else { break };
        let (yes, no) = block.split_once("{else}").unwrap_or((block, ""));
        out.push_str(if holds(cond, ctx) { yes } else { no });
        at = body.len() - next.len();
    }
    out.push_str(&body[at..]);
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
    expand_at(body, ctx, 0, &Cell::new(MAX_LOOKUPS))
}

fn expand_at(body: &str, ctx: &Context, depth: usize, lookups: &Cell<usize>) -> Expanded {
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
                match placeholder(&inner, ctx, depth, lookups) {
                    Some(Piece::Text(text)) => out.push_str(&strip_hidden(&text)), // clipboard, environment and answers are untrusted; stripped before the caret is counted
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
fn placeholder(inner: &str, ctx: &Context, depth: usize, lookups: &Cell<usize>) -> Option<Piece> {
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
        "snippet" if depth < MAX_DEPTH && lookups.get() > 0 => {
            lookups.set(lookups.get() - 1);
            expand_at(&(ctx.lookup)(arg?)?, ctx, depth + 1, lookups).text
        }
        _ => return None,
    };
    modifiers.into_iter().try_fold(text, |text, modifier| modify(&text, modifier)).map(Piece::Text)
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
