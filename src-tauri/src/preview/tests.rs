//! File: preview/tests.rs
//! Purpose: Protect the palette session rule of the preview window: a request carrying another session's number never acts on the window that is open now.
//! Contents: one test that walks the shared pending slot (a single test, so parallel tests cannot interleave on the static).

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
}
