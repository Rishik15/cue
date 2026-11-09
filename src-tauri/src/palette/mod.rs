//! File: palette/mod.rs
//! Purpose: The selection palette's list: every action that applies to the selected text, and the command that previews one row on demand.
//! Contents: Group / Item — one row; selected_row — the selection itself as the first row; build — the rows for a text from a source (saved actions, then exact built-ins whose result differs, then the text-model built-ins); cancel_token — the cancel token of the request a thread works on (the text model gives up a cold start or a unit when it is set); open / cancel — a palette session begins or ends (`epoch`: session numbers only grow); preview_run — compute one row's result for its own session, skipping work that is no longer wanted.
//! Exact results are computed up front (they cost microseconds); text-model results only when the user stops on the row.

use std::cell::Cell;

use serde::Serialize;

use crate::actions::{Action, Section, Source};
use crate::rules::{self, model::Trigger};
use crate::sanitize::strip_hidden;
use crate::{preview, text};

use epoch::EPOCH;

mod epoch;

/// In the order the palette lists them, so sorting rows by group puts them where they are shown.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Captured,
    Selected,
    Rule,
    Clean,
    Convert,
    Extract,
    Ai,
}

fn group(action: Action) -> Group {
    match action.section() {
        Section::Clean => Group::Clean,
        Section::Convert => Group::Convert,
        Section::Extract => Group::Extract,
        Section::Ai => Group::Ai,
    }
}

#[derive(Serialize, Clone)]
pub struct Item {
    /// `rule:<position>:<name>` or `action:<id>`; `preview_run` and `preview_apply` take it back.
    pub id: String,
    pub name: String,
    pub hint: String,
    pub group: Group,
    pub model: bool,
    /// The transformed text; None for a text-model row until it is run.
    pub result: Option<String>,
}

thread_local! {
    /// The (session, request number) the current thread is working on; None when it is not working for the palette.
    static MINE: Cell<Option<(u64, u64)>> = const { Cell::new(None) };
}

/// A cancel token for the palette request this thread is working on: set once its palette closed or a newer request of it came in. A thread that is not working for the palette gets a token that is never set.
/// The text model passes it through a cold start and every unit, so rows the user already passed are skipped and a loading model is given up.
pub fn cancel_token() -> text::Cancel {
    match MINE.get() {
        Some((session, seq)) => text::Cancel::new(move || !EPOCH.wanted(session, seq)),
        None => text::Cancel::never(),
    }
}

/// Marks this thread as working for request `seq` of `session`; the mark clears on every way out, a panic included.
struct Mine;

impl Mine {
    fn set(session: u64, seq: u64) -> Self {
        MINE.set(Some((session, seq)));
        Mine
    }
}

impl Drop for Mine {
    fn drop(&mut self) {
        MINE.set(None);
    }
}

/// A palette opened: its session number, which its page carries on every request.
pub fn open() -> u64 {
    EPOCH.open()
}

/// The palette closed: every running or waiting request counts as superseded, so none keeps the one-slot model busy. The next `open` takes a later session; nothing is ever reset.
pub fn cancel() {
    EPOCH.cancel();
}

/// The first row of a selection's palette: the text as selected, so the window opens on the text and not on the result of some action.
pub fn selected_row(text: &str) -> Item {
    Item {
        id: "selection:text".into(),
        name: "Selected Text".into(),
        hint: "The text as selected".into(),
        group: Group::Selected,
        model: false,
        result: Some(text.to_string()),
    }
}

/// Saved actions that match, then the built-ins. A row whose exact result equals the text is left out: it would change nothing.
/// `app` is the program the text came from, for saved actions scoped to programs; `html` is the formatting the program also copied, when it offered any.
pub fn build(text: &str, app: &str, html: Option<&str>, source: Source) -> Vec<Item> {
    let fits = text.chars().count() <= text::MAX_CHARS; // longer text is refused by the text model
    let saved = rules::matching(Trigger::Selection, text).into_iter().filter(|c| fits || !c.model).filter_map(|c| {
        let result = if c.model {
            None
        } else {
            rules::run_one(c.at, text, app).filter(|o| !o.matched.is_empty() && o.text != text).map(|o| strip_hidden(&o.text))
        };
        (c.model || result.is_some()).then(|| Item {
            id: format!("rule:{}:{}", c.at, c.name),
            name: c.name,
            hint: c.steps,
            group: Group::Rule,
            model: c.model,
            result,
        })
    });
    let mut rows: Vec<Item> = saved.chain(markdown_row(text, html)).chain(builtin(text, fits && text::installed(), source)).collect();
    rows.sort_by_key(|r| r.group); // stable: the order inside a section is kept
    rows
}

/// Formatted text copied from a page or a document, as Markdown: only when the formatting changes something.
fn markdown_row(text: &str, html: Option<&str>) -> Option<Item> {
    let markdown = strip_hidden(&crate::actions::to_markdown(html?));
    (markdown != text && !markdown.is_empty()).then(|| Item {
        id: "markdown".into(),
        name: "Rich Text to Markdown".into(),
        hint: "Keeps headings, bold, links and lists as Markdown.".into(),
        group: Group::Convert,
        model: false,
        result: Some(markdown),
    })
}

/// The built-ins that apply to text from `source`: the exact ones whose result differs from the text, then the text-model ones (only when a text model is installed).
fn builtin(text: &str, with_ai: bool, source: Source) -> Vec<Item> {
    let item = |a: Action, result: Option<String>| Item {
        id: format!("action:{}", a.id()),
        name: a.label().into(),
        hint: a.hint().into(),
        group: group(a),
        model: a.needs_model(),
        result,
    };
    let offered = || Action::all().filter(move |a| a.offered_in(source));
    let exact = offered().filter(|a| !a.needs_model()).filter_map(|a| a.apply(text).map(|r| item(a, Some(strip_hidden(&r)))));
    let ai = offered().filter(|a| with_ai && a.needs_model()).map(|a| item(a, None));
    exact.chain(ai).collect()
}

/// The result of one row for the text the palette of `session` holds, or None when it changes nothing, fails (the failure is already reported) or is no longer wanted (the palette closed, a newer request came in).
#[tauri::command]
pub async fn preview_run(id: String, seq: u64, session: u64) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || run(&id, seq, session)).await.ok().flatten()
}

/// One request: wanted before any work, bound to the text the palette of its own session holds (never another session's), and checked again before the result is given back.
fn run(id: &str, seq: u64, session: u64) -> Option<String> {
    if !EPOCH.admit(session, seq) {
        return None;
    }
    let (text, app) = preview::context(session)?;
    let _lane = EPOCH.enter_lane(session, seq)?;
    let _mine = Mine::set(session, seq);
    let out = match id.split_once(':')? {
        ("rule", tail) => rules::run_one(rules::position_in(tail)?, &text, &app).filter(|o| !o.matched.is_empty()).map(|o| o.text),
        ("action", name) => serde_json::from_value::<Action>(name.into()).ok()?.apply(&text),
        _ => None,
    };
    if !EPOCH.wanted(session, seq) {
        return None;
    }
    out.map(|t| strip_hidden(&t))
}
