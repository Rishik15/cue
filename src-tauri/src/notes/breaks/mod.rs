//! File: notes/breaks/mod.rs
//! Purpose: The full-screen break alert of a pomodoro: a window built when a rest starts and destroyed when it ends, so nothing is resident while idle. It also keeps the no-peek streak.
//! Contents: Owner — the timer (note and generation) a break belongs to; Stage — the window and settings the break acts on (the app's, or a fake in tests); begin — show the alert for a rest of `rest_secs` after `rounds` focus periods; end — that timer's rest is over: count a streak day and close it; cancel — that timer was stopped, restarted or replaced: close it, no streak; peek — the page's Sneak a peek, only for the break it came from; release — clear the owner when it is the caller; break_act — the page's one command (it painted; Sneak a peek; End Pomodoro).
//! The streak and best are written by Rust alone (`notes_break_streak`, `notes_break_best`), so they are not in `settings::validate`.

#[cfg(test)]
pub(in crate::notes) mod fake;
mod window;

use std::sync::{Mutex, MutexGuard};

use tauri::AppHandle;

use crate::{settings, status};

const STREAK: &str = "notes_break_streak";
const BEST: &str = "notes_break_best";

/// The timer a break belongs to: only that timer ends or cancels it, so one note's timer never closes another's break. The page carries it back with each command.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Owner {
    pub note: i64,
    pub generation: u64,
}

/// The owner of the break on screen. Held while the window is built or closed, so a begin and an end never interleave.
pub(in crate::notes) type Held = Mutex<Option<Owner>>;

static HELD: Held = Mutex::new(None);

/// What a break acts on: its window and the streak settings.
pub(in crate::notes) trait Stage {
    /// Is the break window open.
    fn exists(&self) -> bool;
    /// Destroys the break window when open.
    fn close(&self);
    /// Builds the window for `owner`'s rest. Returns an error text when it cannot.
    fn build(&self, owner: Owner, rest_secs: u64, rounds: u32) -> Result<(), String>;
    /// The no-peek streak now.
    fn streak(&self) -> u64;
    /// Saves the streak (and the best one with it); a failure is shown to the user.
    fn save(&self, streak: u64);
    /// Tells the user `message`.
    fn report(&self, message: &str);
}

/// The real break: the app's window and settings.
struct Screen<'a>(&'a AppHandle);

impl Stage for Screen<'_> {
    /// Is the break window open.
    fn exists(&self) -> bool {
        window::exists(self.0)
    }

    /// Destroys the break window when open.
    fn close(&self) {
        window::close(self.0);
    }

    /// Builds the window with its data (rest, rounds, owner, streaks, theme) inside the page.
    fn build(&self, owner: Owner, rest_secs: u64, rounds: u32) -> Result<(), String> {
        let data = serde_json::json!({
            "rest_secs": rest_secs,
            "rounds": rounds,
            "owner": owner,
            "streak": settings::get_u64(STREAK, 0),
            "best": settings::get_u64(BEST, 0),
            "theme": settings::get_theme(),
        });
        window::build(self.0, format!("window.__BREAK__ = {data};"))
    }

    /// The no-peek streak now.
    fn streak(&self) -> u64 {
        settings::get_u64(STREAK, 0)
    }

    /// Saves the streak and the best one in one save. Settings from a newer Cue are left alone; a failed save is shown to the user.
    fn save(&self, streak: u64) {
        if settings::is_newer() {
            return;
        }
        let best = settings::get_u64(BEST, 0).max(streak);
        if let Err(e) = settings::update(&[(STREAK, Some(streak.into())), (BEST, Some(best.into()))]) {
            self.report(&format!("Could not save the no-peek streak: {e}"));
        }
    }

    /// Tells the user `message`.
    fn report(&self, message: &str) {
        status::report(self.0, message);
    }
}

/// The owner of the break on screen, locked.
fn held(held: &Held) -> MutexGuard<'_, Option<Owner>> {
    held.lock().unwrap_or_else(|e| e.into_inner())
}

/// Shows the break alert for a rest of `owner`'s timer. Inputs: app handle, owner, rest seconds, focus rounds done and `live`, asked once the break is locked in: a timer cancelled meanwhile shows nothing. A failure is shown to the user.
pub fn begin(app: &AppHandle, owner: Owner, rest_secs: u64, rounds: u32, live: impl FnOnce() -> bool) {
    begin_on(&Screen(app), &HELD, owner, rest_secs, rounds, live);
}

/// `begin` on any stage and holder.
pub(in crate::notes) fn begin_on(
    stage: &impl Stage,
    holder: &Held,
    owner: Owner,
    rest_secs: u64,
    rounds: u32,
    live: impl FnOnce() -> bool,
) {
    let mut held = held(holder);
    if !live() {
        return;
    }
    stage.close();
    match stage.build(owner, rest_secs, rounds) {
        Ok(()) => *held = Some(owner),
        Err(e) => {
            *held = None;
            stage.report(&format!("Could not show the break screen: {e}"));
        }
    }
}

/// `owner`'s rest ended with the window still up, so it was not peeked at: the streak grows. Another timer's break is left alone.
pub fn end(app: &AppHandle, owner: Owner) {
    end_on(&Screen(app), &HELD, owner);
}

/// `end` on any stage and holder.
pub(in crate::notes) fn end_on(stage: &impl Stage, holder: &Held, owner: Owner) {
    let mut held = held(holder);
    if *held == Some(owner) && stage.exists() {
        stage.save(stage.streak() + 1);
    }
    close_if_owned(stage, &mut held, owner);
}

/// `owner`'s timer was stopped, restarted or replaced during its rest: the break closes and earns nothing. Another timer's break is left alone.
pub fn cancel(app: &AppHandle, owner: Owner) {
    cancel_on(&Screen(app), &HELD, owner);
}

/// `cancel` on any stage and holder.
pub(in crate::notes) fn cancel_on(stage: &impl Stage, holder: &Held, owner: Owner) {
    close_if_owned(stage, &mut held(holder), owner);
}

/// Sneak a peek on the page of `owner`'s break: the streak starts over and the break closes. A page whose break was replaced or cancelled meanwhile changes nothing.
fn peek(stage: &impl Stage, holder: &Held, owner: Owner) {
    let mut held = held(holder);
    if *held == Some(owner) {
        stage.save(0);
    }
    close_if_owned(stage, &mut held, owner);
}

/// Clears `held` when `owner` holds it. Returns whether it did, so only the owner's break is closed.
fn release(held: &mut Option<Owner>, owner: Owner) -> bool {
    (*held == Some(owner)).then(|| *held = None).is_some()
}

/// Closes the break when `owner` holds it.
fn close_if_owned(stage: &impl Stage, held: &mut Option<Owner>, owner: Owner) {
    if release(held, owner) {
        stage.close();
    }
}

#[cfg(test)]
mod tests {
    use super::fake::Fake;
    use super::*;

    const A: Owner = Owner { note: 1, generation: 1 };
    const B: Owner = Owner { note: 2, generation: 2 };

    /// A break is on screen for `owner`: the fake window open, the holder set.
    fn shown(owner: Owner) -> (Fake, Held) {
        let (stage, holder) = (Fake::default(), Mutex::new(None));
        begin_on(&stage, &holder, owner, 300, 1, || true);
        (stage, holder)
    }

    /// The break closes only for the timer that owns it: another note's, or the same note's older run, leaves it alone.
    #[test]
    fn only_the_owner_releases_a_break() {
        let mut held = Some(A);
        assert!(!release(&mut held, B));
        assert!(!release(&mut held, Owner { note: 1, generation: 0 }));
        assert_eq!(held, Some(A));
        assert!(release(&mut held, A));
        assert_eq!(held, None);
        assert!(!release(&mut held, A));
    }

    /// The page's command is async: a synchronous one runs on the UI thread and deadlocks against `begin`, which holds the break lock while it waits for that thread.
    #[test]
    fn the_page_command_is_async() {
        fn is_async<F: std::future::Future>(_: fn(AppHandle, Act, Owner) -> F) {}
        is_async(break_act);
    }

    /// A rest that runs out with the window up counts a day and closes; one that was cancelled (stop, restart, replace) closes and counts nothing.
    #[test]
    fn ending_earns_the_streak_and_cancelling_does_not() {
        let (stage, holder) = shown(A);
        cancel_on(&stage, &holder, A);
        assert_eq!((stage.open(), stage.streak()), (false, 0));
        let (stage, holder) = shown(A);
        end_on(&stage, &holder, A);
        assert_eq!((stage.open(), stage.streak()), (false, 1));
    }

    /// Another timer's end or cancel leaves the break and the streak alone.
    #[test]
    fn a_foreign_timer_changes_nothing() {
        let (stage, holder) = shown(A);
        end_on(&stage, &holder, B);
        cancel_on(&stage, &holder, B);
        assert_eq!((stage.open(), stage.streak()), (true, 0));
    }

    /// A timer cancelled before the break is locked in shows nothing.
    #[test]
    fn a_dead_timer_opens_no_break() {
        let (stage, holder) = (Fake::default(), Mutex::new(None));
        begin_on(&stage, &holder, A, 300, 1, || false);
        assert_eq!((stage.open(), *held(&holder)), (false, None));
    }

    /// Sneak a peek on the page of the break on screen resets the streak and closes it.
    #[test]
    fn a_peek_ends_its_own_break_and_resets_the_streak() {
        let (stage, holder) = shown(A);
        stage.save(4);
        peek(&stage, &holder, A);
        assert_eq!((stage.open(), stage.streak(), *held(&holder)), (false, 0, None));
    }

    /// A peek from the page of a break that was replaced meanwhile (its owner differs) leaves the new break and the streak alone.
    #[test]
    fn a_stale_peek_cannot_close_the_replacement_break() {
        let (stage, holder) = shown(A);
        begin_on(&stage, &holder, B, 300, 1, || true);
        stage.save(4);
        peek(&stage, &holder, A);
        assert_eq!((stage.open(), stage.streak(), *held(&holder)), (true, 4, Some(B)));
    }
}

/// What the break page asks for.
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Act {
    /// The page has painted: show the window.
    Ready,
    /// Sneak a peek: the streak starts over; the pomodoro goes on.
    Peek,
    /// End Pomodoro: close the alert and stop the pomodoro with it.
    End,
}

/// The page's one command. Inputs: the app handle, the act and the owner of the break the page belongs to. Async, so it runs off the UI thread: `begin` holds the break lock while it waits for that thread to build the window, and a synchronous command waiting for the lock there would freeze both. End Pomodoro stops the pomodoros first (their breaks close with them), so a rest announced meanwhile finds its timer gone.
#[tauri::command]
pub async fn break_act(app: AppHandle, act: Act, owner: Owner) {
    match act {
        Act::Ready => window::show(&app),
        Act::Peek => peek(&Screen(&app), &HELD, owner),
        Act::End => super::timer::stop_all_pomodoros(&app),
    }
}
