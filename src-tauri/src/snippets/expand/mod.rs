//! File: snippets/expand/mod.rs
//! Purpose: Expands snippets while the user types: a trigger such as `:sig` typed in any app is erased and replaced by the snippet's text.
//! Contents: sync — bring the expander in line with the saved snippets, folders and settings (start it, update it, or stop it when there is nothing to expand); on_key — one typed key, handled on the watcher's thread;
//! release — the end of an expansion and the replay of the keys typed meanwhile; expand_hit — one expansion, on a short-lived worker; preview — a snippet's text as it would be typed now. Logic lives in `flow` (what a key starts, with `phase` for the stages), `matcher` (when a trigger was typed) and `template` (placeholders).
//! Nothing runs unless the setting is on and a snippet has a trigger: then one watcher thread (see `platform::watch_typing`) that blocks until a key arrives; otherwise there is none. The thread only matches; typing the text
//! happens on a worker, so keys are never held up. It never expands in Cue's own windows, on a skip list, outside a snippet's allowed programs, in a password field, or in an app running as administrator (Windows blocks our input
//! there), and Enter never ends a trigger. The typed characters stay in memory and are never logged.

mod casing;
pub mod flow;
mod lookup;
pub mod matcher;
mod output;
pub mod pattern;
pub mod phase;
pub mod richtext;
mod rules;
mod settle;
pub mod template;
mod timefmt;
mod undo;
mod values;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use tauri::AppHandle;

use super::ask;
use super::check::{program_entry, program_matches};
use super::folders::folders;
use super::model::{effective, Insert, Snippet};
use super::{store, with_db};
use crate::platform::{self, Typed};
use crate::{log, settings, status};
use casing::follow_case;
use flow::{Flow, Next};
use matcher::{Hit, Matcher};
use output::put;
use template::Context;

pub const ENABLED: &str = "snippets_expand";
pub const SKIP_APPS: &str = "snippets_skip_apps";
pub const UNDO: &str = "snippets_undo";
/// Texts up to this long are kept in memory (up to `CACHE_BUDGET` in all); longer ones are read from the file when their trigger fires.
const SMALL_BODY: usize = 4096;
const CACHE_BUDGET: usize = 1024 * 1024;

/// What the expander needs from one snippet when it fires.
struct Meta {
    insert: Insert,
    body: Option<String>,
}

/// The triggers and the snippets they belong to, replaced as a whole whenever something changes.
struct Index {
    matcher: Matcher,
    meta: HashMap<i64, Meta>,
    /// Programs where nothing expands (the user's list).
    skip: Vec<String>,
    /// Backspace right after an expansion takes it back (a setting, off by default).
    undo: bool,
}

/// The expander's state: the triggers and what the keys typed so far have done (see `flow`; its `undo` is the expansion just typed, while Backspace may still take it back).
struct State {
    index: Arc<Index>,
    flow: Flow<undo::Undo>,
}

type Shared = Arc<Mutex<State>>;

/// Held for the length of a `sync`.
static SYNCING: Mutex<()> = Mutex::new(());

/// The running expander; None while it is stopped.
static RUNNING: Mutex<Option<Shared>> = Mutex::new(None);

/// The running expander slot, held for as long as the guard lives; a poisoned lock is still usable.
fn running() -> MutexGuard<'static, Option<Shared>> {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner())
}

/// The expander's state, held for as long as the guard lives; never held across an OS call.
fn lock(shared: &Shared) -> MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// The program list as the setting stores it: one name per line.
pub fn skip_list() -> Vec<String> {
    settings::get_str(SKIP_APPS).unwrap_or_default().lines().map(program_entry).filter(|p| !p.is_empty()).collect()
}

/// Reads the snippets and folders and builds the triggers and per-snippet data. Input: the app. Returns the index or a user-readable error.
fn build_index(app: &AppHandle) -> Result<Index, String> {
    let (snippets, folders) = with_db(app, |c| Ok((store::list_for_index(c, SMALL_BODY)?, folders(c)?)))?;
    let shared = rules::load();
    let rules: Vec<_> = snippets
        .iter()
        .flat_map(|s| effective(s, s.folder_id.and_then(|id| folders.iter().find(|f| f.id == id))))
        .map(|e| shared.apply(e))
        .collect();
    let mut cached = 0;
    let meta = snippets
        .into_iter()
        .map(|s: Snippet| {
            let keep = !s.body.is_empty() && cached + s.body.len() <= CACHE_BUDGET;
            cached += if keep { s.body.len() } else { 0 };
            (s.id, Meta { insert: shared.insert_for(s.insert), body: keep.then_some(s.body) })
        })
        .collect();
    Ok(Index { matcher: Matcher::new(&rules), meta, skip: skip_list(), undo: settings::get_bool(UNDO, false) })
}

/// Call at start and after anything that changes the snippets, the folders or the settings. Reads the database, so call it from a worker thread.
pub fn sync(app: &AppHandle) {
    let _one_at_a_time = SYNCING.lock().unwrap_or_else(|e| e.into_inner());
    let index = if settings::get_bool(ENABLED, true) {
        build_index(app)
    } else {
        Ok(Index { matcher: Matcher::default(), meta: HashMap::new(), skip: Vec::new(), undo: false })
    };
    let index = match index {
        Ok(index) => Arc::new(index),
        Err(e) => return status::report(app, &e),
    };
    let mut slot = running();
    if index.matcher.is_empty() {
        if slot.take().is_some() {
            platform::stop_typing();
            log::note("snippet expansion stopped");
        }
        return;
    }
    match slot.as_ref() {
        Some(shared) => {
            let mut state = lock(shared);
            state.index = index;
            state.flow.tracker.clear();
        }
        None => match start(app, index) {
            Ok(shared) => *slot = Some(shared),
            Err(e) => status::report(app, &e),
        },
    }
}

/// Starts the typing watcher over a fresh state. Input: the app and the first index. Returns the shared state or the watcher's error.
fn start(app: &AppHandle, index: Arc<Index>) -> Result<Shared, String> {
    let shared: Shared = Arc::new(Mutex::new(State { index, flow: Flow::default() }));
    let (handle, state) = (app.clone(), shared.clone());
    platform::watch_typing(Box::new(move |key| on_key(&handle, &state, key)))?;
    log::note("snippet expansion started");
    Ok(shared)
}

/// One key, on the watcher's thread: find out whether it completes a trigger, and hand the typing to a worker.
fn on_key(app: &AppHandle, shared: &Shared, key: Typed) {
    let (next, index) = {
        let mut state = lock(shared);
        let index = state.index.clone();
        (state.flow.key(key, &index.matcher), index)
    };
    dispatch(app, shared, next, index);
}

/// Starts the work a key or the end of an expansion asked for. Input: the app, the state, what to start and the index the decision was made with.
fn dispatch(app: &AppHandle, shared: &Shared, next: Next<undo::Undo>, index: Arc<Index>) {
    match next {
        Next::Held | Next::Nothing => {}
        Next::Undo(undo) => spawn_worker(app, shared, move |app, _| undo::run(app, &undo)),
        Next::Expand(hit) => {
            let detected = Instant::now();
            spawn_worker(app, shared, move |app, shared| expand_hit(app, shared, &index, &hit, detected));
        }
    }
}

/// Runs `work` on a short-lived worker; `work` says whether the target's content is known afterwards (false after a failed delivery). However it ends, the expansion is finished and the keys typed meanwhile are handled by `release`.
fn spawn_worker(app: &AppHandle, shared: &Shared, work: impl FnOnce(&AppHandle, &Shared) -> bool + Send + 'static) {
    let spawned = crate::guard::spawn("expand", 512 * 1024, {
        let (app, shared) = (app.clone(), shared.clone());
        move || {
            let delivered = crate::guard::contain(|| work(&app, &shared)).unwrap_or(false);
            release(&app, &shared, delivered);
        }
    });
    if spawned.is_err() {
        release(app, shared, true);
    }
}

/// Ends the running expansion and replays the keys typed meanwhile as ordinary typing, all under one lock so a newer key can neither come between them nor see the expander idle first (see `flow`); a key that completes a trigger starts its expansion afterwards. After a failed delivery or undo the keys are only counted in the log: the target's content is unknown and replaying could duplicate input.
fn release(app: &AppHandle, shared: &Shared, delivered: bool) {
    let (next, withheld, index) = {
        let mut state = lock(shared);
        let index = state.index.clone();
        let (next, withheld) = state.flow.end(delivered, &index.matcher);
        (next, withheld, index)
    };
    if withheld > 0 {
        log::note(&format!("expansion failed; {withheld} keys typed meanwhile were not tracked"));
    }
    dispatch(app, shared, next, index);
}

/// Whether to leave this moment alone: Cue's own windows, the skip lists, a password field, an app we cannot type into.
fn declined(index: &Index, hit: &Hit) -> bool {
    let exe = platform::foreground_app();
    exe == "cue"
        || index.skip.iter().any(|a| program_matches(a, &exe))
        || !index.matcher.allows(hit.rule, &exe)
        || platform::input_blocked()
        || platform::focused_is_password()
}

/// The values a template can read now. Input: the app, snippet id and body, the answers and captures, and a lookup of other snippets. Returns the context.
fn context<'a>(
    (app, id, body): (&AppHandle, Option<i64>, &str),
    (args, captures): Values,
    others: &'a dyn Fn(&str) -> Option<String>,
) -> Context<'a> {
    Context {
        now: platform::local_time(),
        clipboard: template::uses_clipboard(body).then(platform::clipboard_text).flatten(),
        app: platform::foreground_app(),
        counter: lookup::counter(app, id, body),
        lookup: others,
        args,
        captures,
    }
}

/// The answers to a snippet's `{argument}` questions and the groups its pattern trigger matched, each by name.
type Values = (HashMap<String, String>, HashMap<String, String>);

/// The text a snippet types now. Input: the app, the snippet id (None when not saved), its body and the values. Returns the text and where the caret goes.
fn render(app: &AppHandle, id: Option<i64>, body: &str, values: Values) -> template::Expanded {
    let others = lookup::Others::load(app, body);
    template::expand(body, &context((app, id, body), values, &|name| others.find(name)))
}

/// The snippet's text: from memory when it is small, from the file when it is not.
fn body_of(app: &AppHandle, meta: &Meta, id: i64) -> Option<String> {
    if let Some(body) = &meta.body {
        return Some(body.clone());
    }
    match with_db(app, |c| store::get(c, id)) {
        Ok(snippet) => snippet.map(|s| s.body),
        Err(e) => {
            status::report(app, &e);
            None
        }
    }
}

/// What the user typed for the snippet's `{argument}` questions (none when there are none); None when they cancelled. The window the trigger was typed in gets the focus back before anything is erased.
fn answers(app: &AppHandle, body: &str) -> Option<HashMap<String, String>> {
    let questions = template::arguments(body);
    if questions.is_empty() {
        return Some(HashMap::new());
    }
    let target = platform::foreground_window();
    let answered = ask::ask(app, questions)?;
    if platform::focus_window(target) {
        return Some(answered);
    }
    status::report(app, "Cue could not return to the app, so the snippet was not typed.");
    None
}

/// Where an expansion spent its time (no text, only milliseconds), so a slow one can be found in `cue.log`.
fn note_timing(marks: [Instant; 5]) {
    let ms = |from: usize, to: usize| marks[to].duration_since(marks[from]).as_millis();
    log::note(&format!("expansion: start {} ms, checks {} ms, text {} ms, typing {} ms", ms(0, 1), ms(1, 2), ms(2, 3), ms(3, 4)));
}

/// One expansion, on a worker. Returns whether the target's content is known afterwards: false only when typing the text failed part way. Keys typed since the trigger was completed are behind it already, so sealing drains and settles them in one lock and they are erased with it and typed again after the text; keys after the seal wait (see `phase`).
/// shortcut: the erase count is frozen at the seal, so a key typed after it and before the batch reaches the app lands before our Backspaces (the Backspaces then erase that key and leave the trigger's first character, and the key is lost); the window is the clipboard, focus and held-key steps inside `put`, upgrade by passing a callback into `platform::insert` that re-reads the late keys inside the held-key lift and shrinks or aborts the batch.
fn expand_hit(app: &AppHandle, shared: &Shared, index: &Index, hit: &Hit, detected: Instant) -> bool {
    let started = Instant::now();
    let token = platform::capture_target();
    if token.window == 0 || declined(index, hit) {
        return true;
    }
    let checked = Instant::now();
    let Some(meta) = index.meta.get(&hit.id) else { return true };
    let Some(body) = body_of(app, meta, hit.id) else { return true };
    let Some(args) = answers(app, &body) else { return true };
    let expanded = render(app, Some(hit.id), &body, (args, hit.captures.iter().cloned().collect()));
    let rendered = Instant::now();
    let mut text =
        if hit.changed_case && index.matcher.follows_case(hit.rule) { follow_case(&expanded.text, &hit.typed) } else { expanded.text };
    text.push_str(&hit.retype);
    let Some(meanwhile) = lock(shared).flow.phase.seal() else { return true };
    text.push_str(&meanwhile);
    let kept = hit.retype.chars().count() + meanwhile.chars().count();
    let back = if expanded.back > 0 { expanded.back + kept } else { 0 };
    if let Err(e) = put(hit.erase + meanwhile.chars().count(), &text, back, meta.insert, &token) {
        status::report(app, &e);
        return false;
    }
    note_timing([detected, started, checked, rendered, Instant::now()]);
    if index.undo && meta.insert != Insert::Rich && undo::eligible(&text, expanded.back) {
        let restore = format!("{}{}{meanwhile}", hit.typed, hit.retype);
        lock(shared).flow.undo = Some(undo::Undo { remove: text.chars().count(), restore, insert: meta.insert, token });
    }
    let _ = with_db(app, |c| store::bump(c, hit.id));
    true
}

/// A snippet's text as it would be typed now (placeholders filled in), for the editor and the palette; `id` is None for a snippet not saved yet.
pub fn preview(app: &AppHandle, id: Option<i64>, snippet_body: &str) -> String {
    render(app, id, snippet_body, Default::default()).text
}
