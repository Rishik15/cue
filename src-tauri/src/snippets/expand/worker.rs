//! File: snippets/expand/worker.rs
//! Purpose: The work that follows a typed trigger, off the watcher's thread: start a short-lived worker for an expansion or an undo, wait out the injection delay, decide whether this moment is left alone, ask the `{argument}` questions, type the text, hand the finished expansion back to the state and only then count the use, so the database write never holds up the keys typed meanwhile.
//! Contents: dispatch — start what a key or the end of an expansion asked for; Finished — how a worker's work ended; spawn_worker / release — run the work, end the expansion, then count the use and start what the late keys completed; expand_hit — one expansion; after_delay, declined, answers, note_timing — its steps; own_window_declines / completion_volume — two small decisions, public for the tests.
//! The state itself (`Flow`) is moved only through its methods, under the one lock, and never across an OS call.

use crate::lock::lock;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tauri::AppHandle;

use super::super::ask;
use super::super::check::program_matches;
use super::super::keys;
use super::super::model::Insert;
use super::super::{store, with_db};
use super::casing::follow_case;
use super::flow::{Next, Outcome};
use super::matcher::Hit;
use super::output::{complete, put, Hooks};
use super::{read_body, render, still_current, template, undo, Index, Shared};
use crate::platform;
use crate::sound::{self, Sound};
use crate::{log, settings, status};

/// The volume of the sound for a delivered expansion, read just before the delivery starts so the Notes alert volume applies at once. Input: whether the sound is on and the stored volume. Returns None for silence, otherwise the volume capped at 100.
pub fn completion_volume(on: bool, stored: u64) -> Option<u8> {
    on.then(|| stored.min(100) as u8)
}

/// Starts the work a key or the end of an expansion asked for. Input: the app, the state, what to start and the index the decision was made with.
pub(super) fn dispatch(app: &AppHandle, shared: &Shared, next: Next<undo::Undo>, index: Arc<Index>) {
    match next {
        Next::Held | Next::Nothing => {}
        Next::Undo(undo) => {
            spawn_worker(app, shared, move |app, shared| Finished { outcome: undo::run(app, shared, &undo), counted: None })
        }
        Next::Expand(hit) => {
            let detected = Instant::now();
            spawn_worker(app, shared, move |app, shared| expand_hit(app, shared, &index, hit, detected));
        }
    }
}

/// How a worker's work ended: the outcome for the state, and the snippet whose use to count once the expansion is over.
struct Finished {
    outcome: Outcome<undo::Undo>,
    counted: Option<i64>,
}

/// Runs `work` on a short-lived worker; `work` says how it ended (a panic counts as a failed delivery). However it ends, the expansion is finished, the keys typed meanwhile are handled and a delivered snippet's use is counted by `release`.
fn spawn_worker(app: &AppHandle, shared: &Shared, work: impl FnOnce(&AppHandle, &Shared) -> Finished + Send + 'static) {
    let spawned = crate::guard::spawn("expand", 512 * 1024, {
        let (app, shared) = (app.clone(), shared.clone());
        move || {
            let done = crate::guard::contain(|| work(&app, &shared)).unwrap_or(Finished { outcome: Outcome::Failed, counted: None });
            release(&app, &shared, done);
        }
    });
    if spawned.is_err() {
        release(app, shared, Finished { outcome: Outcome::Delivered(None), counted: None });
    }
}

/// Ends the running expansion and replays the keys typed meanwhile as ordinary typing, all under one lock so a newer key can neither come between them nor see the expander idle first (see `flow`). Only then is the use of a delivered snippet written (`counted`), so the database never holds up the typing, and a key that completed a trigger starts its expansion after that write, so its `{counter}` sees the new count. After a failed delivery or undo the keys are only counted in the log: the target's content is unknown and replaying could duplicate input.
fn release(app: &AppHandle, shared: &Shared, done: Finished) {
    let (next, withheld, index) = {
        let mut state = lock(shared);
        let index = state.index.clone();
        let (next, withheld) = state.flow.end(done.outcome, &index.matcher);
        (next, withheld, index)
    };
    if withheld > 0 {
        log::note(&format!("expansion failed; {withheld} keys typed meanwhile were not tracked"));
    }
    if let Some(id) = done.counted {
        let _ = with_db(app, |c| store::bump(c, id));
    }
    dispatch(app, shared, next, index);
}

/// Whether a Cue window stops expansion: every one does (the snippet editor and settings fields must not fire triggers) except the notes window, a plain text editor. Input: the foreground program's name, the foreground window, the notes window's handle if it exists. Return: the answer.
pub fn own_window_declines(exe: &str, window: isize, notes: Option<isize>) -> bool {
    exe == "cue" && notes != Some(window)
}

/// Whether to leave this moment alone: Cue's own windows except notes, the skip lists, a password field, an app we cannot type into. Input: the app, the index, the hit and the window the trigger was typed in.
fn declined(app: &AppHandle, index: &Index, hit: &Hit, window: isize) -> bool {
    let exe = platform::foreground_app();
    own_window_declines(&exe, window, crate::notes::window_handle(app))
        || index.skip.iter().any(|a| program_matches(a, &exe))
        || !index.matcher.allows(hit.rule, &exe)
        || platform::input_blocked()
        || platform::focused_is_password()
}

/// What the user typed for the snippet's `{argument}` questions (none when there are none); None when they cancelled. The window the trigger was typed in gets the focus back before anything is erased.
fn answers(app: &AppHandle, shared: &Shared, body: &str) -> Option<HashMap<String, String>> {
    let questions = template::arguments(body);
    if questions.is_empty() {
        return Some(HashMap::new());
    }
    let target = platform::foreground_window();
    let answered = ask::ask(app, questions, &|| lock(shared).flow.current())?;
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

/// Waits out the injection delay on the worker (no timer exists while idle): a key typed meanwhile that completes a longer trigger replaces `hit`, and the wait starts again for it. Input: the state, the index and the trigger just completed. Return: the trigger to expand.
fn after_delay(shared: &Shared, index: &Index, mut hit: Hit) -> Hit {
    while !index.delay.is_zero() {
        std::thread::sleep(index.delay);
        match lock(shared).flow.extend(&index.matcher, &hit) {
            Some(Next::Expand(longer)) => hit = longer,
            _ => break,
        }
    }
    hit
}

/// One expansion, on a worker. Returns how it ended, with the snippet to count when its text was typed: failed only when typing the text failed part way, delivered (with the undo it arms, none after a warning such as a clipboard not restored) otherwise, also when nothing was typed because the expansion was declined, stale, cancelled just before the keys (stale, or a key arrived after the seal; a clipboard that could not be given back is reported) or sealed off. Keys typed since the trigger was completed are behind it already, so sealing drains and settles them in one lock and they are erased with it and typed again after the text; keys after the seal wait (see `phase`).
fn expand_hit(app: &AppHandle, shared: &Shared, index: &Index, hit: Hit, detected: Instant) -> Finished {
    let skipped = Finished { outcome: Outcome::Delivered(None), counted: None };
    let started = Instant::now();
    let token = platform::capture_target();
    if token.window == 0 {
        return skipped;
    }
    let hit = after_delay(shared, index, hit);
    if declined(app, index, &hit, token.window) {
        return skipped;
    }
    let hit = &hit;
    let checked = Instant::now();
    let Some(meta) = index.meta.get(&hit.id) else { return skipped };
    let Some(body) = index.text(hit.id, &|id| read_body(app, id)) else { return skipped };
    let Some(args) = answers(app, shared, &body) else { return skipped };
    let expanded = render(app, index, Some(hit.id), &body, (args, hit.captures.iter().cloned().collect()));
    let rendered = Instant::now();
    let mut text =
        if hit.changed_case && index.matcher.follows_case(hit.rule) { follow_case(&expanded.text, &hit.typed) } else { expanded.text };
    text.push_str(&hit.retype);
    let Some(meanwhile) = lock(shared).flow.seal() else { return skipped };
    text.push_str(&meanwhile);
    let kept = hit.retype.chars().count() + meanwhile.chars().count();
    let back = if expanded.back > 0 { expanded.back + kept } else { 0 };
    let (proceed, vetoed) = still_current(shared);
    let hooks = Hooks {
        on_sent: completion_volume(settings::get_bool(keys::SOUND, keys::DEFAULT_SOUND), settings::get_u64("notes_alert_volume", 70))
            .map(|volume| Box::new(move || sound::play(Sound::SnippetDone, volume)) as Box<dyn Fn() + Send + Sync>),
        proceed: Some(proceed),
    };
    let sent = put(hit.erase + meanwhile.chars().count(), &text, back, meta.insert, index.settle, &token, hooks);
    if vetoed.load(Ordering::SeqCst) {
        if let Err(e) = &sent {
            status::report(app, &e.to_string());
        }
        return skipped;
    }
    let done = complete(sent, || {
        (index.undo && meta.insert != Insert::Rich && undo::eligible(&text, expanded.back)).then(|| {
            let restore = format!("{}{}{meanwhile}", hit.typed, hit.retype);
            undo::Undo { remove: text.chars().count(), restore, insert: meta.insert, settle: index.settle, token }
        })
    });
    if let Some(report) = &done.report {
        status::report(app, report);
    }
    if done.counted {
        note_timing([detected, started, checked, rendered, Instant::now()]);
    }
    Finished { outcome: done.outcome, counted: done.counted.then_some(hit.id) }
}
