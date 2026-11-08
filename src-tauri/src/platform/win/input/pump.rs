//! File: win/input/pump.rs
//! Purpose: The one input thread (`cue-input`, 256 KiB, highest priority). It owns the keyboard hook, the mouse hook while a surface needs it, the watcher window and the reducer, and blocks in GetMessage: no polling, and at most one timer, only while a recording is open. Commands wake it with a thread message.
//! Contents: State / with — the thread-local state; request_sync / sync — keep writes, the mouse hook and the timer in step with what is open; reinstall — replace the keyboard hook; start_input_pump / wake_input / PumpHandle — start, wake and stop the thread.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_HIGHEST};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, PostThreadMessageW, SetTimer, TranslateMessage, HHOOK, MSG, WM_APP, WM_QUIT,
};

use super::session::TIMER;
use super::{hook, pointer, session};
use crate::input::{ArmOutcome, Health, Runtime, Sink};

const WM_WAKE: u32 = WM_APP + 1;
const WM_SYNC: u32 = WM_APP + 2;

type Rt = Runtime<Box<dyn Sink>>;

/// Everything the input thread owns.
pub(super) struct State {
    pub(super) rt: Rt,
    keyboard: HHOOK,
    mouse: Option<HHOOK>,
    watcher: Option<HWND>,
    pub(super) timer_at: Option<Instant>,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

/// Runs `f` on this thread's state. None when there is no state, or it is already borrowed (a callback arriving while the thread is inside the reducer passes its key).
pub(super) fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|cell| cell.try_borrow_mut().ok().and_then(|mut guard| guard.as_mut().map(f)))
}

/// Asks the loop to bring the mouse hook and the timer in line with what is open, if they are not. Safe from callbacks: it only posts a message.
pub(super) fn request_sync() {
    let out_of_step =
        with(|st| st.rt.core.needs_pointer() != st.mouse.is_some() || st.rt.core.next_deadline() != st.timer_at || st.rt.core.writes_due())
            .unwrap_or(false);
    if out_of_step {
        unsafe {
            let _ = PostThreadMessageW(GetCurrentThreadId(), WM_SYNC, WPARAM(0), LPARAM(0));
        }
    }
}

/// Installs or removes the mouse hook as what is open requires. A hook that cannot be made ends the surfaces that need it.
fn sync_mouse(st: &mut State) {
    match (st.rt.core.needs_pointer(), st.mouse) {
        (true, None) => match pointer::install() {
            Ok(hook) => st.mouse = Some(hook),
            Err(e) => {
                crate::log::note(&format!("input: {e}"));
                st.rt.fail_surfaces(
                    "Cue could not watch for clicks outside the shortcut box. Close this and try again.".into(),
                    Instant::now(),
                );
            }
        },
        (false, Some(h)) => {
            pointer::uninstall(h);
            st.mouse = None;
        }
        _ => {}
    }
}

/// Sets, moves or kills the one deadline timer so it always matches the nearest deadline. With no watcher window or a timer Windows refuses, the surface that needs a deadline ends, since it could never expire.
fn sync_timer(st: &mut State) {
    let want = st.rt.core.next_deadline();
    if want == st.timer_at {
        return;
    }
    let Some(hwnd) = st.watcher else {
        if want.is_some() {
            st.rt.fail_surfaces("Cue could not set a time limit for this. Close this and try again.".into(), Instant::now());
        }
        return;
    };
    match want {
        Some(at) => {
            let ms = at.saturating_duration_since(Instant::now()).as_millis().clamp(1, u32::MAX as u128) as u32;
            if unsafe { SetTimer(Some(hwnd), TIMER, ms, None) } == 0 {
                st.timer_at = None;
                st.rt.fail_surfaces("Cue could not set a time limit for this. Close this and try again.".into(), Instant::now());
            } else {
                st.timer_at = want;
            }
        }
        None => {
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER);
            }
            st.timer_at = None;
        }
    }
}

/// Hands queued writes to the writer, then brings the mouse hook and the timer in line with what is open.
pub(super) fn sync(st: &mut State) {
    if st.rt.core.writes_due() {
        st.rt.drive_writes(Instant::now());
    }
    sync_mouse(st);
    sync_timer(st);
}

/// The deadline timer fired: it is killed (a Windows timer repeats), the deadlines run, and the timer is set again for what remains.
pub(super) fn timer_fired(st: &mut State) {
    if let Some(hwnd) = st.watcher {
        unsafe {
            let _ = KillTimer(Some(hwnd), TIMER);
        }
    }
    st.timer_at = None;
    st.rt.tick(Instant::now());
    sync(st);
}

/// Installs a new keyboard hook and removes the old one. Health shows Recovering meanwhile; a hook that cannot be made is Failed.
pub(super) fn reinstall(st: &mut State) -> Result<(), String> {
    st.rt.core.set_health(Health::Recovering);
    match hook::install() {
        Ok(new) => {
            hook::uninstall(std::mem::replace(&mut st.keyboard, new));
            st.rt.core.set_health(Health::InstalledUnproven);
            Ok(())
        }
        Err(e) => {
            st.rt.core.set_health(Health::Failed);
            Err(e)
        }
    }
}

/// Handles queued commands, answers a recording's request for a ready hook by reinstalling it, then re-syncs.
fn wake(st: &mut State) {
    st.rt.process(Instant::now());
    if st.rt.recovering() {
        let outcome = reinstall(st);
        st.rt.complete_recover(outcome);
    }
    if st.rt.deferred() {
        let outcome = match reinstall(st) {
            Ok(()) => ArmOutcome::Ready,
            Err(e) => ArmOutcome::Failed(e),
        };
        st.rt.complete_arm(outcome, Instant::now());
    }
    sync(st);
}

/// Dispatches until WM_QUIT.
fn run_loop() {
    let mut msg = MSG::default();
    loop {
        if unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 <= 0 {
            break;
        }
        match msg.message {
            WM_WAKE => {
                let _ = crate::guard::contain(|| with(wake));
            }
            WM_SYNC => {
                let _ = crate::guard::contain(|| with(sync));
            }
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }
}

/// Builds the thread's state: the hooks and the watcher window. Returns the reason on failure.
fn setup(rt: Rt) -> Result<(), String> {
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
    }
    let watcher = session::create();
    if watcher.is_none() {
        crate::log::note("input: the session watcher window could not be made; lock and resume are not noticed");
    }
    let keyboard = hook::install()?;
    STATE.with(|cell| *cell.borrow_mut() = Some(State { rt, keyboard, mouse: None, watcher, timer_at: None }));
    Ok(())
}

/// Removes the hooks, the timer and the window.
fn teardown() {
    let Some(mut st) = STATE.with(|cell| cell.borrow_mut().take()) else { return };
    hook::uninstall(st.keyboard);
    if let Some(mouse) = st.mouse.take() {
        pointer::uninstall(mouse);
    }
    if let Some(hwnd) = st.watcher {
        unsafe {
            let _ = KillTimer(Some(hwnd), TIMER);
        }
        session::destroy(hwnd);
    }
}

/// The running input thread.
pub struct PumpHandle {
    thread: Arc<AtomicU32>,
}

impl PumpHandle {
    /// Ends the thread; its hooks and window are removed on the thread itself.
    pub fn stop(self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread.load(Ordering::Acquire), WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

/// Wakes the input thread to handle queued commands. Does nothing before the thread exists.
pub fn wake_input(thread: u32) {
    if thread != 0 {
        unsafe {
            let _ = PostThreadMessageW(thread, WM_WAKE, WPARAM(0), LPARAM(0));
        }
    }
}

/// Starts the input thread with `rt` and returns once its keyboard hook is installed, or the reason it could not be. `thread` receives the thread id for `wake_input`.
pub fn start_input_pump(thread: Arc<AtomicU32>, rt: Rt) -> Result<PumpHandle, String> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let id = thread.clone();
    crate::guard::spawn("cue-input", 256 * 1024, move || {
        let ready = setup(rt);
        let ok = ready.is_ok();
        id.store(unsafe { GetCurrentThreadId() }, Ordering::Release);
        let _ = ready_tx.send(ready);
        if ok {
            let _ = crate::guard::contain(|| with(wake));
            run_loop();
            teardown();
        }
    })
    .map_err(|e| format!("Cue could not start its keyboard thread: {e}"))?;
    ready_rx.recv().map_err(|_| "Cue's keyboard thread stopped while starting.".to_string())??;
    Ok(PumpHandle { thread })
}
