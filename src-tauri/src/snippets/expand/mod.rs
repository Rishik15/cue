//! File: snippets/expand/mod.rs
//! Purpose: Expands snippets while the user types: a trigger such as `:sig` typed in any app is erased and replaced by the snippet's text.
//! Contents: sync — bring the expander in line with the saved snippets, folders and settings (start it, swap its triggers, or stop it when there is nothing to expand); on_key — one typed key, handled on the watcher's thread; render / preview — a snippet's text as it would be typed now.
//! The work after a trigger is in `worker`; logic lives in `flow` (what a key starts, and the one owner of the typing state, with `phase` for the stages), `matcher` (when a trigger was typed) and `template` (placeholders).
//! Nothing runs unless the setting is on and a snippet has a trigger: then one watcher thread (see `platform::watch_typing`) that blocks until a key arrives; otherwise there is none. The thread only matches; typing the text
//! happens on a worker, so keys are never held up. It never expands in Cue's own windows (the notes window excepted), on a skip list, outside a snippet's allowed programs, in a password field, or in an app running as administrator (Windows blocks our input
//! there), and Enter never ends a trigger. The typed characters stay in memory and are never logged.

mod casing;
pub mod flow;
mod lookup;
pub mod matcher;
pub mod output;
pub mod pattern;
mod phase;
pub mod richtext;
mod settle;
pub mod template;
mod timefmt;
mod undo;
mod values;
pub mod worker;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tauri::AppHandle;

use super::check::program_entry;
use super::folders::folders;
use super::keys;
use super::model::{Insert, Snippet};
use super::policy;
use super::{store, with_db};
use crate::platform::{self, Typed};
use crate::{log, settings, status};
use flow::Flow;
use matcher::Matcher;
use template::Context;
use worker::dispatch;

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
    /// How long a completed trigger waits for a longer one; zero expands at once.
    delay: Duration,
    /// Extra wait of a paste for the target to take the text.
    settle: Duration,
}

impl Index {
    /// The timing and sound settings with the triggers and programs given. Input: the triggers, the per-snippet data and the skip list. Return: the index, the timing settings read once here and bounded as `settings::validate` bounds them.
    fn new(matcher: Matcher, meta: HashMap<i64, Meta>, skip: Vec<String>) -> Index {
        let ms = |key| Duration::from_millis(settings::get_u64(key, 0).min(keys::MAX_MS));
        Index {
            matcher,
            meta,
            skip,
            undo: settings::get_bool(keys::UNDO, keys::DEFAULT_UNDO),
            delay: ms(keys::DELAY_MS),
            settle: ms(keys::RESPONSE_MS),
        }
    }
}

/// The expander's state: the triggers and what the keys typed so far have done (see `flow`; its `undo` is the expansion just typed, while Backspace may still take it back).
struct State {
    index: Arc<Index>,
    flow: Flow<undo::Undo>,
}

impl State {
    /// Swaps the triggers: what was typed, an armed undo and the running expansion's right to type all belong to the old ones.
    fn replace_index(&mut self, index: Arc<Index>) {
        self.index = index;
        self.flow.replace_index();
    }
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
    settings::get_str(keys::SKIP_APPS).unwrap_or_default().lines().map(program_entry).filter(|p| !p.is_empty()).collect()
}

/// Reads the snippets and folders and builds the triggers and per-snippet data. Input: the app. Returns the index or a user-readable error.
fn build_index(app: &AppHandle) -> Result<Index, String> {
    let (snippets, folders) = with_db(app, |c| Ok((store::list_for_index(c, SMALL_BODY)?, folders(c)?)))?;
    let policy = policy::load();
    let rules: Vec<_> =
        snippets.iter().flat_map(|s| policy.resolve(s, s.folder_id.and_then(|id| folders.iter().find(|f| f.id == id)))).collect();
    let mut cached = 0;
    let meta = snippets
        .into_iter()
        .map(|s: Snippet| {
            let keep = !s.body.is_empty() && cached + s.body.len() <= CACHE_BUDGET;
            cached += if keep { s.body.len() } else { 0 };
            (s.id, Meta { insert: s.insert, body: keep.then_some(s.body) })
        })
        .collect();
    Ok(Index::new(Matcher::new(&rules), meta, skip_list()))
}

/// Call at start and after anything that changes the snippets, the folders or the settings. Reads the database, so call it from a worker thread.
pub fn sync(app: &AppHandle) {
    let _one_at_a_time = SYNCING.lock().unwrap_or_else(|e| e.into_inner());
    let index = if settings::get_bool(keys::EXPAND, keys::DEFAULT_EXPAND) {
        build_index(app)
    } else {
        Ok(Index::new(Matcher::default(), HashMap::new(), Vec::new()))
    };
    let index = match index {
        Ok(index) => Arc::new(index),
        Err(e) => return status::report(app, &e),
    };
    let mut slot = running();
    if index.matcher.is_empty() {
        if let Some(stopped) = slot.take() {
            lock(&stopped).flow.stop();
            platform::stop_typing();
            super::ask::close();
            log::note("snippet expansion stopped");
        }
        return;
    }
    match slot.as_ref() {
        Some(shared) => {
            lock(shared).replace_index(index);
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

/// The values a template can read now. Input: the app, snippet id and what the text needs, the answers and captures, and a lookup of other snippets. Returns the context.
fn context<'a>(
    (app, id, needs): (&AppHandle, Option<i64>, &template::Needs),
    (args, captures): Values,
    others: &'a dyn Fn(&str) -> Option<String>,
) -> Context<'a> {
    Context {
        now: platform::local_time(),
        clipboard: needs.clipboard.then(platform::clipboard_text).flatten(),
        app: platform::foreground_app(),
        counter: lookup::counter(app, id, needs.counter),
        lookup: others,
        env: &platform::env_var,
        entropy: &platform::random_u64,
        args,
        captures,
    }
}

/// The answers to a snippet's `{argument}` questions and the groups its pattern trigger matched, each by name.
type Values = (HashMap<String, String>, HashMap<String, String>);

/// The text a snippet types now. Input: the app, the snippet id (None when not saved), its body and the values. Returns the text and where the caret goes.
fn render(app: &AppHandle, id: Option<i64>, body: &str, values: Values) -> template::Expanded {
    let others = lookup::Others::load(app, template::requirements(body, &|_| None).snippets);
    let find = |name: &str| others.find(name);
    let needs = template::requirements(body, &find);
    template::expand(body, &context((app, id, &needs), values, &find))
}

/// A snippet's text as it would be typed now (placeholders filled in), for the editor and the palette; `id` is None for a snippet not saved yet.
pub fn preview(app: &AppHandle, id: Option<i64>, snippet_body: &str) -> String {
    render(app, id, snippet_body, Default::default()).text
}
