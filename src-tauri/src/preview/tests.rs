//! File: preview/tests.rs
//! Purpose: Protect the session rule of the preview window: a request carrying another session's number (a palette's, or an earlier dictation result's) never acts on the window that is open now.
//! Contents: a test that walks the shared pending slot through a palette's session and then a dictation result's; a stale close and an old window's watchdog leaving the newer text alone (all take `SERIAL`, so parallel tests cannot interleave on the statics).

use std::sync::atomic::Ordering;
use std::sync::Mutex;

use super::{is_session, take_applicable, take_if_current, take_session, window::OWNER, Pending, PENDING};
use crate::platform::Target;

/// Held by every test, since they share `PENDING` and `OWNER`.
static SERIAL: Mutex<()> = Mutex::new(());

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
    assert!(is_session(5));
    assert!(!is_session(4));
    assert!(take_applicable(true, 4).is_none());
    assert!(PENDING.lock().unwrap().is_some());
    assert!(take_applicable(true, 5).is_some());
    assert!(!is_session(5));
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
