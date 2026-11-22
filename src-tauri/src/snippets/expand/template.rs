//! File: snippets/expand/template.rs
//! Purpose: Turns a snippet's text into what is typed: placeholders become the date, the clipboard, a counter and so on, and mark where the caret should end up. Pure logic: the time, the clipboard, environment, randomness and other snippets come in through the `Context`.
//! Contents: Context / Expanded — what goes in and out; items / parse — the one reading of `{...}` that discovery and rendering share; arguments / requirements / Needs — what a text asks and needs before it can be typed; expand — the text; placeholder and its helpers — one `{...}` to text.
//! Placeholders: {match} {match:1} (what a `re:` trigger matched), {argument:Label} {argument:Label=a,b} (asked for before typing), {date} {time} {datetime} {weekday} {clipboard} {cursor} {counter} {uuid} {random:a,b,c} {random:1-6} {env:NAME} {app} {snippet:name}, with a layout after a colon ({date:YYYY-MM-DD}, {time:h:mm A}) and modifiers
//! after bars: text ({clipboard | trim | upper}) and offsets for dates ({date | +3d}, {datetime | -2h}; units m h d w M y).
//! Conditions: {if app=slack, teams}...{else}...{/if}, {if app!=slack}, {if clipboard} (one level, no nesting).
//! Written {{ and }} give a literal brace; anything between braces that is not a placeholder stays as typed.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};

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
    /// An environment variable by name, for `{env:NAME}`.
    pub env: &'a dyn Fn(&str) -> Option<String>,
    /// A random number, or None when the system generator fails, for `{uuid}` and `{random:...}`.
    pub entropy: &'a dyn Fn() -> Option<u64>,
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
/// How many distinct snippets discovery reads before it stops; far above what rendering can reach (`MAX_LOOKUPS`).
const MAX_DISCOVERED: usize = 256;

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

/// One piece of a text: a character as typed (an escaped brace is one literal brace) or the inside of a `{...}`. Discovery and rendering both read a text through this, so they cannot disagree on what a placeholder is.
enum Item {
    Char(char),
    Placeholder(String),
}

/// Splits `text` into its characters and placeholders: `{{` and `}}` are literal braces, a placeholder ends at its `}` within `MAX_PLACEHOLDER` characters, and a stray brace is just a character.
fn items(text: &str) -> Vec<Item> {
    let chars: Vec<char> = text.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if (c == '{' && next == Some('{')) || (c == '}' && next == Some('}')) {
            out.push(Item::Char(c));
            i += 2;
            continue;
        }
        let end = (c == '{').then(|| chars[i + 1..].iter().take(MAX_PLACEHOLDER).position(|&x| x == '}' || x == '{')).flatten();
        match end.filter(|&e| chars[i + 1 + e] == '}') {
            Some(e) => {
                out.push(Item::Placeholder(chars[i + 1..i + 1 + e].iter().collect()));
                i += e + 2;
            }
            None => {
                out.push(Item::Char(c));
                i += 1;
            }
        }
    }
    out
}

/// A placeholder's inside, read: `name`, `name:argument`, either followed by `| modifier` items. The name is trimmed and lowercase.
struct Parsed<'a> {
    name: String,
    arg: Option<&'a str>,
    bars: Vec<&'a str>,
}

/// Reads the inside of a placeholder.
fn parse(inner: &str) -> Parsed<'_> {
    let mut parts = inner.split('|').map(str::trim);
    let head = parts.next().unwrap_or_default();
    let (name, arg) = head.split_once(':').map_or((head, None), |(n, l)| (n.trim(), Some(l.trim())));
    Parsed { name: name.to_lowercase(), arg, bars: parts.collect() }
}

/// The questions a text asks, in order of first appearance, each label once. (Arguments inside a nested `{snippet:...}` are not asked.)
pub fn arguments(body: &str) -> Vec<Argument> {
    let mut out: Vec<Argument> = Vec::new();
    for item in items(body) {
        let Item::Placeholder(inner) = item else { continue };
        let Parsed { name, arg: Some(spec), .. } = parse(&inner) else { continue };
        let (label, choices) = split_spec(spec);
        if name != "argument" || label.is_empty() || out.len() >= MAX_ARGUMENTS || out.iter().any(|a| a.label == label) {
            continue;
        }
        let choices = choices.split(',').map(str::trim).filter(|c| !c.is_empty()).map(String::from).collect();
        out.push(Argument { label: label.into(), choices });
    }
    out
}

/// What reading a text's values will need: the clipboard, the other snippets, the use count.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Needs {
    pub clipboard: bool,
    pub snippets: bool,
    pub counter: bool,
}

/// What `body` needs, including what the snippets it includes need (`lookup` finds them). Each included snippet is read once however often or deeply it is named, so cycles end and the work is bounded by the number of snippets, not by branching. Over-asks when a condition hides a placeholder or a limit of rendering would cut a branch, never under-asks.
pub fn requirements(body: &str, lookup: &dyn Fn(&str) -> Option<String>) -> Needs {
    let mut needs = Needs::default();
    discover(body, lookup, &mut HashSet::new(), &mut needs);
    needs
}

/// Adds what `body` needs to `needs`; `seen` holds the lowercased names already read.
fn discover(body: &str, lookup: &dyn Fn(&str) -> Option<String>, seen: &mut HashSet<String>, needs: &mut Needs) {
    for segment in segments(body) {
        match segment {
            Segment::Text(text) => discover_text(text, lookup, seen, needs),
            Segment::Block { cond, yes, no } => {
                needs.clipboard |= cond.trim().eq_ignore_ascii_case("clipboard");
                discover(yes, lookup, seen, needs);
                discover(no, lookup, seen, needs);
            }
        }
    }
}

/// Adds what the placeholders of `text` (no condition block in it) need to `needs`.
fn discover_text(text: &str, lookup: &dyn Fn(&str) -> Option<String>, seen: &mut HashSet<String>, needs: &mut Needs) {
    for item in items(text) {
        let Item::Placeholder(inner) = item else { continue };
        let Parsed { name, arg, .. } = parse(&inner);
        match name.as_str() {
            "clipboard" => needs.clipboard = true,
            "counter" => needs.counter = true,
            "snippet" => {
                needs.snippets = true;
                // shortcut: snippets past MAX_DISCOVERED distinct names are not read, so one that only they reach under-asks; upgrade if libraries that large chain includes
                if let Some(arg) = arg.filter(|a| seen.len() < MAX_DISCOVERED && seen.insert(a.to_lowercase())) {
                    if let Some(text) = lookup(arg) {
                        discover(&text, lookup, seen, needs);
                    }
                }
            }
            _ if name.strip_prefix("if ").is_some_and(|cond| cond.trim() == "clipboard") => needs.clipboard = true,
            _ => {}
        }
    }
}

/// A text cut at its whole `{if}` blocks: the text between them and each block's parts.
enum Segment<'a> {
    Text(&'a str),
    Block { cond: &'a str, yes: &'a str, no: &'a str },
}

/// Cuts `body` at each `{if cond}yes{else}no{/if}` (`{else}` is optional). One level only: blocks do not nest; a block that is not closed stays as typed text. `{{if` is a literal brace, not a condition.
fn segments(body: &str) -> Vec<Segment<'_>> {
    let (mut out, mut from, mut at) = (Vec::new(), 0, 0);
    while let Some(found) = body[at..].find("{if ") {
        let start = at + found;
        if body[..start].chars().rev().take_while(|&c| c == '{').count() % 2 == 1 {
            at = start + 1;
            continue;
        }
        let Some((cond, tail)) = body[start + 4..].split_once('}') else { break };
        let Some((block, next)) = tail.split_once("{/if}") else { break };
        let (yes, no) = block.split_once("{else}").unwrap_or((block, ""));
        out.push(Segment::Text(&body[from..start]));
        out.push(Segment::Block { cond, yes, no });
        from = body.len() - next.len();
        at = from;
    }
    out.push(Segment::Text(&body[from..]));
    out
}

/// Keeps the branch of each `{if cond}yes{else}no{/if}` that holds.
fn resolve_ifs(body: &str, ctx: &Context) -> String {
    segments(body)
        .into_iter()
        .map(|segment| match segment {
            Segment::Text(text) => text,
            Segment::Block { cond, yes, no } => {
                if holds(cond, ctx) {
                    yes
                } else {
                    no
                }
            }
        })
        .collect()
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

/// What `body` becomes when typed. Input: the text and the values it reads. Returns the text and where the caret goes.
pub fn expand(body: &str, ctx: &Context) -> Expanded {
    expand_at(body, ctx, 0, &Cell::new(MAX_LOOKUPS))
}

/// Expands `body` at `depth`: the branches of its conditions that hold, then each placeholder in turn. Returns the text and where the caret goes.
fn expand_at(body: &str, ctx: &Context, depth: usize, lookups: &Cell<usize>) -> Expanded {
    let (mut out, mut cursor) = (String::new(), None::<usize>);
    for item in items(&resolve_ifs(body, ctx)) {
        match item {
            Item::Char(c) => out.push(c),
            Item::Placeholder(inner) => match placeholder(&inner, ctx, depth, lookups) {
                Some(Piece::Text(text)) => out.push_str(&strip_hidden(&text)), // clipboard, environment and answers are untrusted; stripped before the caret is counted
                Some(Piece::Cursor) => {
                    cursor.get_or_insert(out.chars().count());
                }
                None => out.push_str(&format!("{{{inner}}}")),
            },
        }
    }
    let total = out.chars().count();
    Expanded { text: out, back: cursor.map_or(0, |at| total - at) }
}

enum Piece {
    Text(String),
    Cursor,
}

/// One placeholder's inside to its text or the caret mark; None when it is not a placeholder (it then stays as typed).
fn placeholder(inner: &str, ctx: &Context, depth: usize, lookups: &Cell<usize>) -> Option<Piece> {
    let Parsed { name, arg, bars } = parse(inner);
    let (offsets, modifiers): (Vec<_>, Vec<_>) = bars.into_iter().partition(|m| parse_offset(m).is_some());
    let now = offsets.iter().filter_map(|m| parse_offset(m)).fold(ctx.now, |t, (n, unit)| shift(&t, n, unit));
    let text = match name.as_str() {
        "cursor" => return Some(Piece::Cursor),
        "date" => format_time(arg.unwrap_or("D MMM YYYY"), &now),
        "time" => format_time(arg.unwrap_or("HH:mm"), &now),
        "datetime" => format_time(arg.unwrap_or("D MMM YYYY HH:mm"), &now),
        "weekday" => format_time(arg.unwrap_or("dddd"), &now),
        "clipboard" => ctx.clipboard.clone().unwrap_or_default(),
        "counter" => format!("{:0width$}", ctx.counter, width = arg.and_then(|a| a.parse().ok()).unwrap_or(0).min(20)),
        "uuid" => uuid(ctx.entropy)?,
        "random" => random(arg?, ctx.entropy)?,
        "env" => (ctx.env)(arg?).unwrap_or_default(),
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
