//! File: snippets/expand/mod.rs
//! Purpose: Expands snippets while the user types: a trigger such as `:sig` typed in any app is erased and replaced by the snippet's text.
//! Contents: sync — bring the expander in line with the saved snippets, folders and settings (start it, swap its triggers, or stop it when there is nothing to expand); on_key — one typed key, handled on the watcher's thread; Index — the triggers and what an expansion reads (texts kept in memory, include names and the uses that order them); count — the use-count write ordered before `{counter}` reads; render / preview — a snippet's text as it would be typed now.
//! The work after a trigger is in `worker`; logic lives in `flow` (what a key starts, and the one owner of the typing state, with `phase` for the stages), `matcher` (when a trigger was typed) and `template` (placeholders).
//! Nothing runs unless the setting is on and a snippet has a trigger: then one watcher thread (see `platform::watch_typing`) that blocks until a key arrives; otherwise there is none. The thread only matches; typing the text
//! happens on a worker, so keys are never held up. It never expands in Cue's own windows (the notes window excepted), on a skip list, outside a snippet's allowed programs, in a password field, or in an app running as administrator (Windows blocks our input
//! there), and Enter never ends a trigger. The typed characters stay in memory and are never logged.

mod casing;
pub(in crate::snippets) mod count;
pub mod flow;
pub(in crate::snippets) mod lookup;
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

use crate::lock::lock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::Connection;
use tauri::AppHandle;

use super::check::{program_entry, MAX_PATTERNS};
use super::folders::folders;
use super::keys;
use super::model::{Folder, Insert, Snippet};
use super::policy;
use super::{store, with_db};
use crate::platform::{self, Typed};
use crate::{log, settings, status};
use flow::Flow;
use matcher::Matcher;
use pattern::Pattern;
use template::Context;
use worker::dispatch;

/// Texts up to this long are kept in memory (up to `CACHE_BUDGET` in all); longer ones are read from the file when their trigger fires.
const SMALL_BODY: usize = 4096;
const CACHE_BUDGET: usize = 1024 * 1024;

/// What the expander needs from one snippet when it fires.
struct Meta {
    insert: Insert,
    body: Option<String>,
    /// How often it was used: read with the index and raised by every expansion counted since, so include precedence follows the uses.
    uses: AtomicI64,
}

/// The triggers and the snippets they belong to, replaced as a whole whenever something changes.
pub(in crate::snippets) struct Index {
    matcher: Matcher,
    meta: HashMap<i64, Meta>,
    /// What a `{snippet:name}` include is found by (see `lookup::names`), so an include reads nothing from disk unless its text is too big to keep.
    by_name: HashMap<String, Vec<i64>>,
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
    /// The index of `snippets` (in `store::list` order) under the folders and the page's rules, with the skip list and the timing settings read once here and bounded as `settings::validate` bounds them. Input: the snippets, folders and skip list. Return: the index.
    fn of(snippets: Vec<Snippet>, folders: &[Folder], skip: Vec<String>) -> Index {
        let policy = policy::load();
        let rules: Vec<_> =
            snippets.iter().flat_map(|s| policy.resolve(s, s.folder_id.and_then(|id| folders.iter().find(|f| f.id == id)))).collect();
        let patterns = rules.iter().filter(|e| Pattern::new(&e.trigger).is_some()).count();
        if patterns > MAX_PATTERNS {
            log::note(&format!("{patterns} pattern triggers are stored, only {MAX_PATTERNS} fire"));
        }
        let by_name = lookup::names(&snippets);
        let mut cached = 0;
        let meta = snippets
            .into_iter()
            .map(|s: Snippet| {
                let keep = !s.body.is_empty() && cached + s.body.len() <= CACHE_BUDGET;
                cached += if keep { s.body.len() } else { 0 };
                (s.id, Meta { insert: s.insert, body: keep.then_some(s.body), uses: AtomicI64::new(s.uses) })
            })
            .collect();
        let ms = |key| Duration::from_millis(settings::get_u64(key, 0).min(keys::MAX_MS));
        Index {
            matcher: Matcher::new(&rules),
            meta,
            by_name,
            skip,
            undo: settings::get_bool(keys::UNDO, keys::DEFAULT_UNDO),
            delay: ms(keys::DELAY_MS),
            settle: ms(keys::RESPONSE_MS),
        }
    }

    /// Reads the snippets and folders through `conn` and indexes them. Input: an open connection. Returns the index or a user-readable error.
    pub(in crate::snippets) fn read(conn: &Connection) -> Result<Index, String> {
        Ok(Index::of(store::list_for_index(conn, SMALL_BODY)?, &folders(conn)?, skip_list()))
    }

    /// The text of snippet `id`: from memory when it is kept, otherwise from `read` (the file). Input: the id and the reader. Returns None for an unknown id or a failed read.
    fn text(&self, id: i64, read: &dyn Fn(i64) -> Option<String>) -> Option<String> {
        match &self.meta.get(&id)?.body {
            Some(body) => Some(body.clone()),
            None => read(id),
        }
    }

    /// The text a `{snippet:name}` includes: of the enabled snippets with this name or trigger (ASCII case ignored), the most used now, ties to the first in the order the index was read in (`store::list`). Input: the name and the reader for a text not kept in memory. Returns None when none matches.
    pub(in crate::snippets) fn include(&self, name: &str, read: &dyn Fn(i64) -> Option<String>) -> Option<String> {
        let uses = |id: &&i64| self.meta.get(id).map_or(0, |m| m.uses.load(Ordering::Relaxed));
        self.text(*self.by_name.get(&name.to_ascii_lowercase())?.iter().rev().max_by_key(uses)?, read)
    }

    /// Records one more use of snippet `id`, as `store::bump` writes it, so includes keep the order of the uses without a new index. Input: the id.
    pub(in crate::snippets) fn counted(&self, id: i64) {
        if let Some(meta) = self.meta.get(&id) {
            meta.uses.fetch_add(1, Ordering::Relaxed);
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
    lock(&RUNNING)
}

/// The last check before a worker's keys go out: whether the running expansion or undo may still send (see `Flow::may_send`: still current and no key arrived since the seal). Input: the state. Returns a hook for `Hooks::proceed` and a flag that says the hook cancelled the delivery (stale or keys arrived), so the worker can tell that apart from a failure. shortcut: the state lock is released before the keys go out, so a stop landing between this check and the send still lets that one batch through; closing it needs the lock held across the send (several calls with settle sleeps) which would stall the key hook, and the check is already after the slow UI Automation target query. Upgrade if a measured stray paste after a stop shows up.
fn still_current(shared: &Shared) -> (Box<dyn Fn() -> bool + Send + Sync>, Arc<AtomicBool>) {
    let (shared, vetoed) = (shared.clone(), Arc::new(AtomicBool::new(false)));
    let flag = vetoed.clone();
    let hook = Box::new(move || {
        let send = lock(&shared).flow.may_send();
        flag.store(!send, Ordering::SeqCst);
        send
    });
    (hook, vetoed)
}

/// The program list as the setting stores it: one name per line.
pub fn skip_list() -> Vec<String> {
    settings::get_str(keys::SKIP_APPS).unwrap_or_default().lines().map(program_entry).filter(|p| !p.is_empty()).collect()
}

/// Reads the snippets and folders and builds the triggers and per-snippet data. Input: the app. Returns the index or a user-readable error.
fn build_index(app: &AppHandle) -> Result<Index, String> {
    with_db(app, Index::read)
}

/// The text of snippet `id` from the file, for one too big to keep in memory; a failed read is reported. Input: the app and the id. Returns the text, None when it is gone or unreadable.
fn read_body(app: &AppHandle, id: i64) -> Option<String> {
    match with_db(app, |c| store::get(c, id)) {
        Ok(snippet) => snippet.map(|s| s.body),
        Err(e) => {
            status::report(app, &e);
            None
        }
    }
}

/// Call at start and after anything that changes the snippets, the folders or the settings. Reads the database, so call it from a worker thread.
pub fn sync(app: &AppHandle) {
    let _one_at_a_time = lock(&SYNCING);
    let index =
        if settings::get_bool(keys::EXPAND, keys::DEFAULT_EXPAND) { build_index(app) } else { Ok(Index::of(Vec::new(), &[], Vec::new())) };
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

/// The text a snippet types now; includes are answered from `index`. Input: the app, the index, the snippet id (None when not saved), its body and the values. Returns the text and where the caret goes.
fn render(app: &AppHandle, index: &Index, id: Option<i64>, body: &str, values: Values) -> template::Expanded {
    let find = |name: &str| index.include(name, &|id| read_body(app, id));
    let needs = template::requirements(body, &find);
    template::expand(body, &context((app, id, &needs), values, &find))
}

/// A snippet's text as it would be typed now (placeholders filled in), for the editor and the palette; `id` is None for a snippet not saved yet. Includes come from an index built for this call (an unreadable library includes nothing).
pub fn preview(app: &AppHandle, id: Option<i64>, snippet_body: &str) -> String {
    let index = build_index(app).unwrap_or_else(|_| Index::of(Vec::new(), &[], Vec::new()));
    render(app, &index, id, snippet_body, Default::default()).text
}
