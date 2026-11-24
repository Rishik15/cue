//! File: preview/tests.rs
//! Purpose: Protect the session rule of the preview window: a request carrying another session's number (a palette's, or an earlier dictation result's) never acts on the window that is open now.
//! Contents: a test that publication waits for a retired window's destruction; a test that walks the shared pending slot through a palette's session and then a dictation result's; a stale close and an old window's watchdog leaving the newer text alone; an explicit close retiring the watchdog and owner (all take `SERIAL`, so parallel tests cannot interleave on the statics).

use std::sync::atomic::Ordering;
use std::sync::Mutex;

use super::{
    applicable, finish, is_session, release, retire, settle, store,
    window::{OWNER, WATCH},
    Pending, CLOSING, PENDING,
};
use crate::platform::Target;

/// Held by every test, since they share `PENDING` and `OWNER`.
static SERIAL: Mutex<()> = Mutex::new(());

/// What Paste of `session` takes through the one retire path.
fn take_applicable(has_text: bool, session: u64) -> Option<Pending> {
    retire(|p| applicable(p, has_text, session), || None).and_then(|(p, w)| {
        finish(w);
        p
    })
}

/// What a close of `session` takes through the one retire path.
fn take_session(session: u64) -> Option<Pending> {
    retire(|p| is_session(p, session), || None).and_then(|(p, w)| {
        finish(w);
        p
    })
}

/// What the watchdog of window `owner` takes through the one retire path: Some only when it is the current window.
fn take_if_current(owner: u64) -> Option<Option<Pending>> {
    retire(|_| OWNER.load(Ordering::Acquire) == owner, || None).map(|(p, w)| {
        finish(w);
        p
    })
}

/// A dictation result of `session` holding `text`.
fn dictated(session: u64, text: &str) -> Pending {
    Pending {
        original: "um".into(),
        result: Some(text.into()),
        via: String::new(),
        items: None,
        app: String::new(),
        target: Target::default(),
        keep: true,
        copy: false,
        session,
    }
}

/// A palette of session 5 is open: an empty or a full Apply of session 4 (a closed palette's delayed click) is not for it and leaves it in place, session 5 takes it.
#[test]
fn a_request_of_another_session_does_not_touch_the_open_palette() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    *PENDING.lock().unwrap() = Some(Pending {
        original: "text".into(),
        result: None,
        via: String::new(),
        items: Some(Vec::new()),
        app: String::new(),
        target: Target::default(),
        keep: false,
        copy: false,
        session: 5,
    });
    assert!(is_session(PENDING.lock().unwrap().as_ref(), 5));
    assert!(!is_session(PENDING.lock().unwrap().as_ref(), 4));
    assert!(take_applicable(true, 4).is_none());
    assert!(PENDING.lock().unwrap().is_some());
    assert!(take_applicable(true, 5).is_some());
    assert!(!is_session(PENDING.lock().unwrap().as_ref(), 5));
    a_dictation_result_is_applied_only_by_its_own_session();
}

/// A dictation result has a session of its own: the Paste of an earlier result's page (older number, or none) does not apply the newer result, and the newest result's own Paste does.
fn a_dictation_result_is_applied_only_by_its_own_session() {
    let result = |session| Pending {
        original: "um hello".into(),
        result: Some("Hello".into()),
        via: String::new(),
        items: None,
        app: String::new(),
        target: Target::default(),
        keep: true,
        copy: false,
        session,
    };
    *PENDING.lock().unwrap() = Some(result(8));
    assert!(take_applicable(true, 0).is_none(), "a page that sent no session");
    assert!(take_applicable(true, 7).is_none(), "the earlier result's page");
    assert!(PENDING.lock().unwrap().is_some());
    assert!(take_applicable(true, 8).is_some());
    assert!(PENDING.lock().unwrap().is_none());
}

/// Result A (session 4) is replaced by B (session 5) before A's page closes: A's close takes nothing and B stays; B's own close takes it.
#[test]
fn a_stale_close_leaves_the_newer_text() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    *PENDING.lock().unwrap() = Some(dictated(4, "A"));
    PENDING.lock().unwrap().replace(dictated(5, "B"));
    assert!(take_session(4).is_none());
    assert_eq!(PENDING.lock().unwrap().as_ref().and_then(|p| p.result.clone()), Some("B".into()));
    assert_eq!(take_session(5).and_then(|p| p.result), Some("B".into()));
    assert!(take_session(5).is_none());
}

/// The watchdog of window 1 fires after window 2 was built: it takes nothing, and the current window's own watchdog takes the text.
#[test]
fn an_old_windows_watch_leaves_the_new_pending() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    OWNER.store(2, Ordering::Release);
    *PENDING.lock().unwrap() = Some(dictated(9, "new"));
    assert!(take_if_current(1).is_none());
    assert!(PENDING.lock().unwrap().is_some());
    assert_eq!(take_if_current(2).flatten().and_then(|p| p.result), Some("new".into()));
}

/// A preview closed explicitly before its 10 s deadline: the watchdog never fires and the owner is forgotten, so a late `stuck_if` check of the old window finds nothing to report.
#[test]
fn an_explicit_close_retires_the_watchdog_and_owner() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let fired = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let seen = fired.clone();
    OWNER.store(3, Ordering::Release);
    *PENDING.lock().unwrap() = Some(dictated(6, "A"));
    WATCH.arm("test-preview-watch", 3, std::time::Duration::from_millis(50), move |o| seen.store(o, Ordering::SeqCst)).unwrap();
    assert!(take_session(6).is_some());
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(fired.load(Ordering::SeqCst), 0, "the watchdog was retired");
    assert_eq!(OWNER.load(Ordering::Acquire), 0);
    assert!(take_if_current(3).is_none(), "the old owner no longer matches");
}

/// A stale close of session 4 leaves window 2's watchdog and owner alone while session 5 is open.
#[test]
fn a_stale_close_keeps_the_newer_windows_guard() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    OWNER.store(2, Ordering::Release);
    *PENDING.lock().unwrap() = Some(dictated(5, "B"));
    assert!(take_session(4).is_none());
    assert_eq!(OWNER.load(Ordering::Acquire), 2);
}

/// Session A is retired but its window not yet destroyed: publishing B waits, so B can never reuse the doomed window; once A's destruction finishes, B is stored.
#[test]
fn a_text_is_not_published_while_a_retired_window_is_being_destroyed() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    *PENDING.lock().unwrap() = Some(dictated(4, "A"));
    let (_, window) = retire(|p| is_session(p, 4), || None).unwrap();
    let (done, published) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        store(dictated(5, "B"));
        done.send(()).unwrap();
    });
    assert!(published.recv_timeout(std::time::Duration::from_millis(150)).is_err(), "B waited for A's destruction");
    assert!(PENDING.lock().unwrap().is_none());
    assert!(CLOSING.load(Ordering::Acquire));
    finish(window);
    published.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    assert_eq!(PENDING.lock().unwrap().as_ref().map(|p| p.session), Some(5));
}

/// Destruction is requested but the Destroyed event has not arrived: publishing B stays blocked until it does, and a stray Destroyed never underflows the count.
#[test]
fn a_text_waits_for_the_destroyed_event_not_the_destroy_request() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    *PENDING.lock().unwrap() = Some(dictated(4, "A"));
    retire(|p| is_session(p, 4), || ()).unwrap(); // the destroy request returned; `finish` leaves the count to the event
    let (done, published) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        store(dictated(5, "B"));
        done.send(()).unwrap();
    });
    assert!(published.recv_timeout(std::time::Duration::from_millis(150)).is_err(), "B waited for the Destroyed event");
    release(); // the Destroyed event
    published.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    release();
    assert!(!CLOSING.load(Ordering::Acquire));
}

/// The watchdog and a dismiss both retire the same window before its one Destroyed event: the second retire adds nothing, so that single event releases publication.
#[test]
fn retiring_one_window_twice_needs_one_destroyed_event() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    *PENDING.lock().unwrap() = Some(dictated(4, "A"));
    retire(|_| true, || ()).unwrap();
    retire(|_| true, || ()).unwrap();
    release(); // the one Destroyed event
    assert!(!CLOSING.load(Ordering::Acquire));
    store(dictated(5, "B"));
    assert_eq!(PENDING.lock().unwrap().as_ref().map(|p| p.session), Some(5));
}

/// A text waiting to publish is stored only after the Destroyed cleanup has taken the old text: the cleanup never takes B.
#[test]
fn the_destroyed_cleanup_finishes_before_a_waiting_text_is_stored() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    *PENDING.lock().unwrap() = Some(dictated(4, "A"));
    retire(|_| true, || ()).unwrap();
    let (done, published) = std::sync::mpsc::channel();
    let mut worker = None;
    settle(false, |dropped| {
        assert_eq!(dropped.and_then(|p| p.result), Some("A".into()));
        worker = Some(std::thread::spawn(move || {
            store(dictated(5, "B"));
            done.send(()).unwrap();
        }));
        assert!(published.recv_timeout(std::time::Duration::from_millis(150)).is_err(), "B waited for the cleanup");
    });
    published.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    worker.unwrap().join().unwrap();
    assert_eq!(PENDING.lock().unwrap().as_ref().map(|p| p.session), Some(5));
}
