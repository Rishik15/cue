//! File: preview/tests.rs
//! Purpose: Protect the session rule of the preview window: a request carrying another session's number (a palette's, or an earlier dictation result's) never acts on the window that is open now.
//! Contents: one test that walks the shared pending slot through a palette's session and then a dictation result's (a single test, so parallel tests cannot interleave on the static).

use super::{is_session, take_applicable, Pending, PENDING};
use crate::platform::Target;

/// A palette of session 5 is open: an empty or a full Apply of session 4 (a closed palette's delayed click) is not for it and leaves it in place, session 5 takes it.
#[test]
fn a_request_of_another_session_does_not_touch_the_open_palette() {
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
