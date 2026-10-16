//! Purpose: The selection palette's list: every action that applies to the selected text, and the command that previews one row on demand.
//! Contents: Group / Item — one row; build — the rows for a text from a source (saved actions, then exact built-ins whose result differs, then the text-model built-ins); reset / preview_run — compute one row's result, skipping work a newer request made stale.
//! Exact results are computed up front (they cost microseconds); text-model results only when the user stops on the row.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::actions::{Action, Section, Source};
use crate::rules::{self, model::Trigger};
use crate::sanitize::strip_hidden;
use crate::{preview, text};

/// In the order the palette lists them, so sorting rows by group puts them where they are shown.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Captured,
    Rule,
    Clean,
    Convert,
    Extract,
    Ai,
    Snippet,
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

/// The newest request number seen, so a request that waited behind a newer one is skipped instead of occupying the one-slot model.
static LATEST: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The request number the current thread is working on (u64::MAX when it is not working for the palette).
    static MINE: Cell<u64> = const { Cell::new(u64::MAX) };
}

/// True when a newer preview request came in after this thread's one started: the text model checks it once it has the model to itself, so rows the user already passed are skipped.
pub fn superseded() -> bool {
    let mine = MINE.get();
    mine != u64::MAX && LATEST.load(Ordering::Acquire) > mine
}

pub fn reset() {
    LATEST.store(0, Ordering::Release);
}

/// Saved actions that match, then the built-ins. A row whose exact result equals the text is left out: it would change nothing.
/// `app` is the program the text came from, for saved actions scoped to programs; `html` is the formatting the program also copied, when it offered any.
pub fn build(text: &str, app: &str, html: Option<&str>, source: Source) -> Vec<Item> {
    let saved = rules::matching(Trigger::Selection, text).into_iter().filter_map(|c| {
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
    let mut rows: Vec<Item> = saved.chain(markdown_row(text, html)).chain(builtin(text, text::installed(), source)).collect();
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

/// The result of one row for the text the window holds, or None when it changes nothing, fails (the failure is already reported) or was superseded.
#[tauri::command]
pub async fn preview_run(id: String, seq: u64) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || run(&id, seq)).await.ok().flatten()
}

fn run(id: &str, seq: u64) -> Option<String> {
    if LATEST.fetch_max(seq, Ordering::AcqRel) > seq {
        return None;
    }
    let (text, app) = preview::context()?;
    MINE.set(seq);
    let out = match id.split_once(':')? {
        ("rule", tail) => rules::run_one(rules::position_in(tail)?, &text, &app).filter(|o| !o.matched.is_empty()).map(|o| o.text),
        ("action", name) => serde_json::from_value::<Action>(name.into()).ok()?.apply(&text),
        _ => None,
    };
    MINE.set(u64::MAX);
    out.map(|t| strip_hidden(&t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_rows_that_change_nothing_are_left_out_and_ai_rows_follow() {
        let rows = builtin("HELLO   WORLD", true, Source::Selection);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"action:clean_whitespace") && ids.contains(&"action:lower_case"));
        assert!(!ids.contains(&"action:upper_case"), "already upper case");
        assert!(rows.iter().filter(|r| r.group != Group::Ai).all(|r| r.result.is_some()));
        assert!(rows.iter().filter(|r| r.group == Group::Ai).all(|r| r.result.is_none() && r.model));
        let first_ai = rows.iter().position(|r| r.group == Group::Ai).unwrap();
        assert!(rows[first_ai..].iter().all(|r| r.group == Group::Ai), "quick rows come first");
    }

    #[test]
    fn ai_rows_are_left_out_without_a_text_model() {
        assert!(builtin("some text", false, Source::Selection).iter().all(|r| r.group != Group::Ai));
    }

    #[test]
    fn text_from_the_screen_is_not_offered_spoken_clean_up() {
        let ids = |source| builtin("um so  hello hello", false, source).into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert!(ids(Source::Selection).contains(&"action:remove_fillers".to_string()));
        assert!(!ids(Source::Capture).contains(&"action:remove_fillers".to_string()));
    }

    #[test]
    fn a_request_that_waited_behind_a_newer_one_is_skipped() {
        reset();
        LATEST.store(5, Ordering::Release);
        assert_eq!(run("action:upper_case", 3), None);
    }
}
