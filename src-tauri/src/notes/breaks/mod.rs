//! File: notes/breaks/mod.rs
//! Purpose: The full-screen break alert of a pomodoro: a window built when a rest starts and destroyed when it ends, so nothing is resident while idle. It also keeps the no-peek streak.
//! Contents: Owner — the timer (note and generation) a break belongs to; Shown — the break on screen: its owner and its own serial; Stage — the window and settings the break acts on (the app's, or a fake in tests); Screen — the app's stage; begin — show the alert for a rest of `rest_secs` after `rounds` focus periods; end — that timer's rest is over: count a streak day and close it; cancel_on — that timer was stopped, restarted or replaced: close it, no streak; peek — the page's Sneak a peek, only for the break (serial) it came from; release — clear the owner when it is the caller; break_act — the page's one command (it painted; Sneak a peek; End Pomodoro).
//! The streak and best are written by Rust alone (`notes_break_streak`, `notes_break_best`), so they are not in `settings_door::validate`.

#[cfg(test)]
pub(in crate::notes) mod fake;
mod window;

use crate::lock::lock;
use std::sync::atomic::{AtomicU64, Ordering};
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

/// The break on screen: the timer it belongs to (who may end or cancel it) and its own serial. The serial is new for every break, even each rest of one timer, so a page action sent by an earlier break never acts on a later one.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::notes) struct Shown {
    pub owner: Owner,
    pub serial: u64,
}

/// The break on screen. Held while the window is built or closed, so a begin and an end never interleave.
pub(in crate::notes) type Held = Mutex<Option<Shown>>;

pub(in crate::notes) static HELD: Held = Mutex::new(None);

/// The serial of the last break built.
static SERIAL: AtomicU64 = AtomicU64::new(0);

/// What a break acts on: its window and the streak settings.
pub(in crate::notes) trait Stage {
    /// Is the break window open.
    fn exists(&self) -> bool;
    /// Destroys the break window when open.
    fn close(&self);
    /// Builds the window for the break `serial` of `rest_secs` after `rounds` focus periods. Returns an error text when it cannot.
    fn build(&self, serial: u64, rest_secs: u64, rounds: u32) -> Result<(), String>;
    /// The no-peek streak now.
    fn streak(&self) -> u64;
    /// Saves the streak (and the best one with it); a failure is shown to the user.
    fn save(&self, streak: u64);
    /// Tells the user `message`.
    fn report(&self, message: &str);
}

/// The real break: the app's window and settings.
pub(in crate::notes) struct Screen<'a>(pub &'a AppHandle);

impl Stage for Screen<'_> {
    /// Is the break window open.
    fn exists(&self) -> bool {
        window::exists(self.0)
    }

    /// Destroys the break window when open.
    fn close(&self) {
        window::close(self.0);
    }

    /// Builds the window with its data (rest, rounds, serial, streaks, theme) inside the page.
    fn build(&self, serial: u64, rest_secs: u64, rounds: u32) -> Result<(), String> {
        let data = serde_json::json!({
            "rest_secs": rest_secs,
            "rounds": rounds,
            "serial": serial,
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

/// The break on screen, locked.
fn held(held: &Held) -> MutexGuard<'_, Option<Shown>> {
    lock(held)
}

/// Shows the break alert for a rest of `owner`'s timer. Inputs: app handle, owner, rest seconds, focus rounds done and `live`, asked once the break is locked in: a timer cancelled meanwhile shows nothing. A failure is shown to the user.
pub fn begin(app: &AppHandle, owner: Owner, rest_secs: u64, rounds: u32, live: impl FnOnce() -> bool) {
    begin_on(&Screen(app), &HELD, owner, rest_secs, rounds, live);
}

/// `begin` on any stage and holder. Returns the serial of the break shown, None when none was.
pub(in crate::notes) fn begin_on(
    stage: &impl Stage,
    holder: &Held,
    owner: Owner,
    rest_secs: u64,
    rounds: u32,
    live: impl FnOnce() -> bool,
) -> Option<u64> {
    let mut held = held(holder);
    if !live() {
        return None;
    }
    stage.close();
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed) + 1;
    match stage.build(serial, rest_secs, rounds) {
        Ok(()) => {
            *held = Some(Shown { owner, serial });
            Some(serial)
        }
        Err(e) => {
            *held = None;
            stage.report(&format!("Could not show the break screen: {e}"));
            None
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
    if owned(&held, owner) && stage.exists() {
        stage.save(stage.streak() + 1);
    }
    close_if_owned(stage, &mut held, owner);
}

/// `owner`'s timer was stopped, restarted or replaced during its rest: the break closes and earns nothing. Another timer's break is left alone. On any stage and holder.
pub(in crate::notes) fn cancel_on(stage: &impl Stage, holder: &Held, owner: Owner) {
    close_if_owned(stage, &mut held(holder), owner);
}

/// Sneak a peek on the page of break `serial`: the streak starts over and the break closes. A page whose break was replaced, cancelled or followed by a later rest of the same timer changes nothing.
fn peek(stage: &impl Stage, holder: &Held, serial: u64) {
    let mut held = held(holder);
    let Some(shown) = held.filter(|s| s.serial == serial) else { return };
    stage.save(0);
    close_if_owned(stage, &mut held, shown.owner);
}

/// Does `owner`'s timer own the break on screen.
fn owned(held: &Option<Shown>, owner: Owner) -> bool {
    held.is_some_and(|s| s.owner == owner)
}

/// Clears `held` when `owner` holds it. Returns whether it did, so only the owner's break is closed.
fn release(held: &mut Option<Shown>, owner: Owner) -> bool {
    owned(held, owner).then(|| *held = None).is_some()
}

/// Closes the break when `owner` holds it.
fn close_if_owned(stage: &impl Stage, held: &mut Option<Shown>, owner: Owner) {
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

    /// A break is on screen for `owner`: the fake window open, the holder set. Returns them and the break's serial.
    fn shown(owner: Owner) -> (Fake, Held, u64) {
        let (stage, holder) = (Fake::default(), Mutex::new(None));
        let serial = begin_on(&stage, &holder, owner, 300, 1, || true).unwrap();
        (stage, holder, serial)
    }

    /// The owner of the break on screen.
    fn on_screen(holder: &Held) -> Option<Owner> {
        held(holder).map(|s| s.owner)
    }

    /// The break closes only for the timer that owns it: another note's, or the same note's older run, leaves it alone.
    #[test]
    fn only_the_owner_releases_a_break() {
        let mut held = Some(Shown { owner: A, serial: 1 });
        assert!(!release(&mut held, B));
        assert!(!release(&mut held, Owner { note: 1, generation: 0 }));
        assert_eq!(held.map(|s| s.owner), Some(A));
        assert!(release(&mut held, A));
        assert!(held.is_none());
        assert!(!release(&mut held, A));
    }

    /// The page's command is async: a synchronous one runs on the UI thread and deadlocks against `begin`, which holds the break lock while it waits for that thread.
    #[test]
    fn the_page_command_is_async() {
        fn is_async<F: std::future::Future>(_: fn(AppHandle, Act, u64) -> F) {}
        is_async(break_act);
    }

    /// A rest that runs out with the window up counts a day and closes; one that was cancelled (stop, restart, replace) closes and counts nothing.
    #[test]
    fn ending_earns_the_streak_and_cancelling_does_not() {
        let (stage, holder, _) = shown(A);
        cancel_on(&stage, &holder, A);
        assert_eq!((stage.open(), stage.streak()), (false, 0));
        let (stage, holder, _) = shown(A);
        end_on(&stage, &holder, A);
        assert_eq!((stage.open(), stage.streak()), (false, 1));
    }

    /// Another timer's end or cancel leaves the break and the streak alone.
    #[test]
    fn a_foreign_timer_changes_nothing() {
        let (stage, holder, _) = shown(A);
        end_on(&stage, &holder, B);
        cancel_on(&stage, &holder, B);
        assert_eq!((stage.open(), stage.streak()), (true, 0));
    }

    /// A timer cancelled before the break is locked in shows nothing.
    #[test]
    fn a_dead_timer_opens_no_break() {
        let (stage, holder) = (Fake::default(), Mutex::new(None));
        begin_on(&stage, &holder, A, 300, 1, || false);
        assert_eq!((stage.open(), on_screen(&holder)), (false, None));
    }

    /// Sneak a peek on the page of the break on screen resets the streak and closes it.
    #[test]
    fn a_peek_ends_its_own_break_and_resets_the_streak() {
        let (stage, holder, serial) = shown(A);
        stage.save(4);
        peek(&stage, &holder, serial);
        assert_eq!((stage.open(), stage.streak(), on_screen(&holder)), (false, 0, None));
    }

    /// A peek from the page of an earlier break leaves a later one and the streak alone: another timer's, and a later rest of the same timer (same note and generation).
    #[test]
    fn a_stale_peek_cannot_close_a_later_break() {
        for later in [B, A] {
            let (stage, holder, first) = shown(A);
            end_on(&stage, &holder, A);
            let second = begin_on(&stage, &holder, later, 300, 1, || true).unwrap();
            assert_ne!(first, second);
            stage.save(4);
            peek(&stage, &holder, first);
            assert_eq!((stage.open(), stage.streak(), on_screen(&holder)), (true, 4, Some(later)));
            peek(&stage, &holder, second);
            assert_eq!((stage.open(), stage.streak(), on_screen(&holder)), (false, 0, None));
        }
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

/// The page's one command. Inputs: the app handle, the act and the serial of the break the page belongs to. Async, so it runs off the UI thread: `begin` holds the break lock while it waits for that thread to build the window, and a synchronous command waiting for the lock there would freeze both. End Pomodoro stops the pomodoros first (their breaks close with them), so a rest announced meanwhile finds its timer gone.
#[tauri::command]
pub async fn break_act(app: AppHandle, act: Act, serial: u64) {
    match act {
        Act::Ready => window::show(&app),
        Act::Peek => peek(&Screen(&app), &HELD, serial),
        Act::End => super::timer::stop_all_pomodoros(&app),
    }
}
