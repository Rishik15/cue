//! File: snippets/expand/mod.rs
//! Purpose: Expands snippets while the user types: a trigger such as `:sig` typed in any app is erased and replaced by the snippet's text.
//! Contents: sync / publish — bring the expander in line with the saved snippets, folders and settings (start it, swap its triggers, or stop it when there is nothing to expand or the library cannot be read; a file locked by another program is tried once more after `RETRY_AFTER`, decided by `retrying`); stop — end the running expander; on_key — one typed key, handled on the watcher's thread; Index — the triggers and what an expansion reads (texts kept in memory, include names and the uses that order them); count / count_use — the use-count write, ordered before `{counter}` reads and against index swaps; render / expand_reading / preview — a snippet's text as it would be typed now, refused when a text it needs cannot be read.
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
use std::cell::RefCell;
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
use super::store::{self, Failure};
use super::{db_path, with_db, WRITES};
use crate::platform::{self, Typed};
use crate::{log, settings, status};
use flow::Flow;
use matcher::Matcher;
use template::Context;
use worker::dispatch;

/// Texts up to this long are kept in memory (up to `CACHE_BUDGET` in all); longer ones are read from the file when their trigger fires.
pub(in crate::snippets) const SMALL_BODY: usize = 4096;
const CACHE_BUDGET: usize = 1024 * 1024;
/// How long one index read waits for another program's lock on the file: short, so a read and its one retry end within a few seconds.
const INDEX_WAIT: Duration = Duration::from_secs(1);
/// The pause before the one retry of an index read that found the file locked.
const RETRY_AFTER: Duration = Duration::from_millis(500);

/// What the expander needs from one snippet when it fires.
struct Meta {
    insert: Insert,
    body: Option<String>,
    /// How often it was used: read with the index and raised by every expansion counted since, so include precedence follows the uses.
    uses: AtomicI64,
    /// The name folded as SQLite's `NOCASE` folds it (ASCII only), the tie-break of equal uses in `store::list`.
    folded: String,
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
    /// The index of `snippets` (in `store::list` order) under the folders and the page's rules, with the skip list and the timing settings read once here and bounded as `settings_door::validate` bounds them. Input: the snippets, folders and skip list. Return: the index.
    fn of(snippets: Vec<Snippet>, folders: &[Folder], skip: Vec<String>) -> Index {
        let rules = store::resolve_all(&snippets, folders, &policy::load());
        let by_name = lookup::names(&snippets);
        let mut cached = 0;
        let meta = snippets
            .into_iter()
            .map(|s: Snippet| {
                let keep = !s.body.is_empty() && cached + s.body.len() <= CACHE_BUDGET;
                cached += if keep { s.body.len() } else { 0 };
                (
                    s.id,
                    Meta {
                        insert: s.insert,
                        folded: s.name.to_ascii_lowercase(),
                        body: keep.then_some(s.body),
                        uses: AtomicI64::new(s.uses),
                    },
                )
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

    /// Reads the snippets and folders through `conn` in one snapshot and indexes them. Input: an open connection outside a transaction. Returns the index, or the failure: SQLite's own when the file cannot be read, a refusal when (edited by hand) it holds more enabled pattern triggers than writes allow.
    pub(in crate::snippets) fn read(conn: &Connection) -> Result<Index, Failure> {
        let index =
            store::snapshot(conn, |c| Ok::<_, Failure>(Index::of(store::list_for_index(c, SMALL_BODY)?, &folders(c)?, skip_list())))?;
        match index.matcher.pattern_count() {
            n if n > MAX_PATTERNS => Err(Failure::Refused(format!(
                "The snippet library holds {n} pattern triggers (re:), and Cue runs {MAX_PATTERNS} at most, so snippets do not expand. Turn some off on the Snippets page."
            ))),
            _ => Ok(index),
        }
    }

    /// The message for `re:` triggers left out because they do not compile (only a hand edit stores one). Returns it, None when every pattern compiled.
    pub(in crate::snippets) fn broken_note(&self) -> Option<String> {
        match self.matcher.broken_count() {
            0 => None,
            n => Some(format!("{n} pattern triggers (re:) are not valid patterns and never expand. Fix them on the Snippets page.")),
        }
    }

    /// The text of snippet `id`: from memory when it is kept, otherwise from `read` (the file). Input: the id and the reader. Returns None for an unknown id or a failed read.
    fn text(&self, id: i64, read: &dyn Fn(i64) -> Option<String>) -> Option<String> {
        match &self.meta.get(&id)?.body {
            Some(body) => Some(body.clone()),
            None => read(id),
        }
    }

    /// The text a `{snippet:name}` includes: of the enabled snippets with this name or trigger (ASCII case ignored), the first in the order `store::list` would give now (`uses DESC, name COLLATE NOCASE`, with the uses counted since the read), full ties to the order the index was read in. Input: the name and the reader for a text not kept in memory. Returns None when none matches.
    pub(in crate::snippets) fn include(&self, name: &str, read: &dyn Fn(i64) -> Option<String>) -> Option<String> {
        let uses = |m: &Meta| m.uses.load(Ordering::Relaxed);
        let (id, _) = self
            .by_name
            .get(&name.to_ascii_lowercase())?
            .iter()
            .filter_map(|id| Some((*id, self.meta.get(id)?)))
            .min_by(|(_, a), (_, b)| uses(b).cmp(&uses(a)).then_with(|| a.folded.cmp(&b.folded)))?;
        self.text(id, read)
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

/// Reads the snippets and folders and builds the triggers and per-snippet data, waiting at most `INDEX_WAIT` per statement for another program's lock. Input: the app. Returns the index or the failure, SQLite's error kept typed.
fn build_index(app: &AppHandle) -> Result<Index, Failure> {
    Index::read(&store::open_waiting(&db_path(app)?, INDEX_WAIT)?)
}

/// The text of snippet `id` from the file, for one too big to keep in memory. Input: the app and the id. Returns the text, None when it is gone, or the user-readable error when it cannot be read.
fn read_body(app: &AppHandle, id: i64) -> Result<Option<String>, String> {
    Ok(with_db(app, |c| store::get(c, id))?.map(|s| s.body))
}

/// Writes one use of snippet `id` and tells the running index, so include precedence follows the uses. Called holding the count gate (`count`); `publish` reads again when a use was written during its read and holds the gate for its swap, so the use is either in what that read saw or counted on the index that replaced it. Input: the app and the id. Returns the database error.
fn count_use(app: &AppHandle, id: i64) -> Result<(), String> {
    with_db(app, |c| store::bump(c, id))?;
    if let Some(shared) = running().as_ref() {
        lock(shared).index.counted(id);
    }
    Ok(())
}

/// Call at start and after anything that changes the snippets, the folders or the settings. Reads the database, so call it from a worker thread.
/// A library that cannot be indexed stops the expander, so no trigger the file no longer holds keeps firing; when another program held the file locked (SQLITE_BUSY) it stays stopped for `RETRY_AFTER` and the whole read is tried once more, then the failure is reported once. Damage, a refused version or the pattern limit are reported at once. No timer is left behind. `re:` triggers that do not compile are reported and the rest still expand.
pub fn sync(app: &AppHandle) {
    let _one_at_a_time = lock(&SYNCING);
    let done = retrying(
        || publish(app),
        || {
            stop(&mut running());
            std::thread::sleep(RETRY_AFTER);
        },
    );
    if let Err(e) = done {
        stop(&mut running());
        let message = if e.is_busy() {
            format!("Snippets stopped expanding: another program kept the snippets file locked ({e}). Close it, then turn snippet expansion off and on.")
        } else {
            e.to_string()
        };
        status::report(app, &message);
    }
}

/// Runs `attempt`; when it found the file locked by another program (SQLITE_BUSY), runs `pause` and tries once more. Any other failure is returned at once. Input: the attempt and the pause between the two. Returns the last attempt's result.
pub(in crate::snippets) fn retrying(mut attempt: impl FnMut() -> Result<(), Failure>, pause: impl FnOnce()) -> Result<(), Failure> {
    match attempt() {
        Err(e) if e.is_busy() => {
            pause();
            attempt()
        }
        done => done,
    }
}

/// One attempt of `sync`: reads the index and puts it in place (or stops the expander when it has no trigger). The read and the page's rules are taken under the snippet write lock (one state that writes left); the read runs outside the count gate, so an expansion's end never waits for it, and the gate is held only for the swap (`count::swap`: no use count lands between the read and the swap). Input: the app. Returns the read's failure; nothing changes then.
fn publish(app: &AppHandle) -> Result<(), Failure> {
    let writes = lock(&WRITES);
    let expand = settings::get_bool(keys::EXPAND, keys::DEFAULT_EXPAND);
    let (index, counts) = count::swap(|| if expand { build_index(app) } else { Ok(Index::of(Vec::new(), &[], Vec::new())) })?;
    let note = index.broken_note();
    let started = if index.matcher.is_empty() {
        drop((counts, writes));
        stop(&mut running());
        Ok(())
    } else {
        let mut slot = running();
        let index = Arc::new(index);
        match slot.as_ref() {
            Some(shared) => {
                lock(shared).replace_index(index);
                Ok(())
            }
            None => start(app, index).map(|shared| *slot = Some(shared)),
        }
    };
    for message in note.into_iter().chain(started.err()) {
        status::report(app, &message);
    }
    Ok(())
}

/// Stops the running expander, if any: its pending work goes stale, the watcher ends and an open `{argument}` dialog closes. Input: the held slot.
fn stop(slot: &mut Option<Shared>) {
    if let Some(stopped) = slot.take() {
        lock(&stopped).flow.stop();
        platform::stop_typing();
        super::ask::close();
        log::note("snippet expansion stopped");
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

/// The values a template can read now. Input: the app, snippet id and what the text needs, the answers and captures, and a lookup of other snippets. Returns the context, or the error when the use count cannot be read (or an earlier one is still unsaved).
fn context<'a>(
    (app, id, needs): (&AppHandle, Option<i64>, &template::Needs),
    (args, captures): Values,
    others: &'a dyn Fn(&str) -> Option<String>,
) -> Result<Context<'a>, String> {
    Ok(Context {
        now: platform::local_time(),
        clipboard: needs.clipboard.then(platform::clipboard_text).flatten(),
        app: platform::foreground_app(),
        counter: lookup::counter(app, id, needs.counter)?,
        lookup: others,
        env: &platform::env_var,
        entropy: &platform::random_u64,
        args,
        captures,
    })
}

/// The answers to a snippet's `{argument}` questions and the groups its pattern trigger matched, each by name.
type Values = (HashMap<String, String>, HashMap<String, String>);

/// The text a snippet types now; includes are answered from `index`. Input: the app, the index, the snippet id (None when not saved), its body and the values. Returns the text and where the caret goes, or the error when an included text or the use count could not be read: the text would be incomplete or its number wrong, so nothing is typed.
fn render(app: &AppHandle, index: &Index, id: Option<i64>, body: &str, values: Values) -> Result<template::Expanded, String> {
    let read = |id| read_body(app, id);
    expand_reading(index, body, &read, |needs, find| Ok(template::expand(body, &context((app, id, needs), values, find)?)))
}

/// Expands a text whose includes come from `index`, texts not kept in memory read through `read`: `expand` gets what the text needs and the include lookup and types it. Any failed read refuses the whole text (a gone snippet is only a missing include). Inputs: the index, the text, the reader and the expansion. Returns the expansion, or the first read error.
pub(in crate::snippets) fn expand_reading(
    index: &Index,
    body: &str,
    read: &dyn Fn(i64) -> Result<Option<String>, String>,
    expand: impl FnOnce(&template::Needs, &dyn Fn(&str) -> Option<String>) -> Result<template::Expanded, String>,
) -> Result<template::Expanded, String> {
    let failed = RefCell::new(None);
    let kept = |id| {
        read(id).unwrap_or_else(|e| {
            failed.borrow_mut().get_or_insert(e);
            None
        })
    };
    let find = |name: &str| index.include(name, &kept);
    let expanded = expand(&template::requirements(body, &find), &find)?;
    failed.into_inner().map_or(Ok(expanded), Err)
}

/// A snippet's text as it would be typed now (placeholders filled in), for the editor and the palette; `id` is None for a snippet not saved yet. Includes come from an index built for this call. Returns the text, or the user-readable error, also reported, when the library cannot be indexed or a text it needs cannot be read (the editor then shows the text as written).
pub fn preview(app: &AppHandle, id: Option<i64>, snippet_body: &str) -> Result<String, String> {
    let index = build_index(app).map_err(String::from).inspect_err(|e| status::report(app, e))?;
    Ok(render(app, &index, id, snippet_body, Default::default()).inspect_err(|e| status::report(app, e))?.text)
}
