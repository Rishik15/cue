//! File: platform/tests/delivery.rs
//! Purpose: Protect what the user is told when text could not be delivered: whether the text reached the app or sits on the clipboard, the line on the pill for each outcome, how a receipt becomes that outcome, and when a target token still matches.
//! Contents: tests of `InsertError`, `InsertOptions`, `Receipt` and `Target`.

use crate::platform::{Caret, ClipboardEnd, Failure, InsertError, InsertOptions, Placement, Receipt, Submit, Target};

/// Every way a delivery can end. Returns one of each kind (and both copied states where they matter).
fn outcomes() -> Vec<InsertError> {
    vec![
        InsertError::Elevated,
        InsertError::FocusMoved { copied: true },
        InsertError::FocusMoved { copied: false },
        InsertError::Failed { why: "no focus".into(), copied: true },
        InsertError::Failed { why: "no focus".into(), copied: false },
        InsertError::Uncertain { why: "SendInput injected 3/12".into(), copied: true },
        InsertError::Uncertain { why: "SendInput injected 3/12".into(), copied: false },
        InsertError::NotRestored,
        InsertError::KeepCopy,
        InsertError::Submit("blocked".into()),
        InsertError::Caret("focus moved".into()),
    ]
}

/// The pill and the text-went-in flag agree for every outcome: errors where the text went in say "Inserted", the others say it could not be inserted and whether it was copied.
#[test]
fn the_pill_says_where_the_text_went() {
    for error in outcomes() {
        let pill = error.pill();
        assert_eq!(error.text_went_in(), pill.starts_with("Inserted"), "{error:?}: {pill}");
        assert_eq!(pill.starts_with("Could not insert, copied"), error.copied(), "{error:?}: {pill}");
    }
}

/// The status line names the reason, and a failure message from lower down says the text was not saved until the caller learns otherwise.
#[test]
fn the_status_line_gives_the_reason_and_the_clipboard_state() {
    assert_eq!(
        InsertError::Failed { why: "no focus".into(), copied: true }.to_string(),
        "Could not insert text: no focus, so the text is on your clipboard"
    );
    assert_eq!(InsertError::Submit("blocked".into()).to_string(), "The text was inserted, but Cue could not press the submit key: blocked");
    let plain: InsertError = String::from("boom").into();
    assert!(matches!(&plain, InsertError::Failed { copied: false, why } if why == "boom"));
    assert!(plain.to_string().ends_with("the text could not be saved to the clipboard"));
    for error in outcomes() {
        assert!(!error.to_string().is_empty());
    }
}

/// A delivery with no options typed pastes as fits, restores the user's clipboard and presses nothing after.
#[test]
fn the_default_delivery_restores_the_clipboard_and_presses_nothing() {
    let options = InsertOptions::default();
    assert!(!options.always_paste && options.restore_clipboard && !options.keep_copy && !options.slow);
    assert_eq!((options.target, options.submit), (0, Submit::None));
    assert!(options.token.is_none() && options.fit.is_none());
}

/// A sent text with a failed caret move, clipboard restore, copy or submit is reported as sent, one fact each; nothing is called confirmed.
#[test]
fn a_sent_text_reports_each_later_failure_separately() {
    let sent = || Receipt { placement: Placement::Sent, ..Receipt::nothing() };
    assert!(sent().into_result().is_ok());
    let caret = Receipt { caret: Caret::NotDone("typed".into()), ..sent() };
    assert!(matches!(caret.into_result(), Err(InsertError::Caret(_))));
    let left = Receipt { clipboard: ClipboardEnd::LeftBehind("too large".into()), ..sent() };
    assert!(matches!(left.into_result(), Err(InsertError::NotRestored)));
    let kept = Receipt { clipboard: ClipboardEnd::Kept, ..sent() };
    assert!(kept.into_result().is_ok());
    assert!(matches!(Receipt { keep_copy_failed: true, ..sent() }.into_result(), Err(InsertError::KeepCopy)));
    assert!(matches!(Receipt { submit: Some("x".into()), ..sent() }.into_result(), Err(InsertError::Submit(_))));
}

/// A text that was not sent says why and whether it is on the clipboard; a partial send is uncertain and a pasted text nobody had read yet is uncertain, never "not taken" (the app may still insert it).
#[test]
fn an_unsent_text_says_why_and_where_it_is() {
    let nothing = Receipt::nothing();
    assert_eq!(nothing.placement, Placement::NotDone);
    let moved = Receipt { failure: Some(Failure::FocusMoved), copied: true, ..Receipt::nothing() };
    assert!(matches!(moved.into_result(), Err(InsertError::FocusMoved { copied: true })));
    let partial = Receipt {
        placement: Placement::Uncertain,
        failure: Some(Failure::Other("SendInput injected 3/12".into())),
        copied: true,
        ..Receipt::nothing()
    };
    assert!(matches!(partial.into_result(), Err(InsertError::Uncertain { copied: true, .. })));
    let unread = Receipt { placement: Placement::Uncertain, failure: Some(Failure::NotRead), copied: true, ..Receipt::nothing() };
    match unread.into_result() {
        Err(e @ InsertError::Uncertain { copied: true, .. }) => assert!(!e.to_string().contains("did not take")),
        other => panic!("{other:?}"),
    }
    let busy = Receipt { failure: Some(Failure::Busy), ..Receipt::nothing() };
    assert!(matches!(busy.into_result(), Err(InsertError::Failed { copied: false, .. })));
}

/// An app that rebuilds the focused element keeps its control type and automation id: the same key is the same field, a different or empty key is not.
#[test]
fn a_rebuilt_element_with_the_same_key_is_the_same_field() {
    let token = Target { window: 10, pid: 4, thread: 5, runtime_id: Some(vec![42, 7]), key: "50004:editor".into(), ..Target::default() };
    let rebuilt = Target { runtime_id: Some(vec![42, 9]), ..token.clone() };
    assert!(token.same_as(&rebuilt));
    assert!(!token.same_as(&Target { key: "50004:title".into(), ..rebuilt.clone() }));
    assert!(!token.same_as(&Target { runtime_id: None, ..rebuilt.clone() }));
    let unnamed = Target { key: String::new(), ..token.clone() };
    assert!(!unnamed.same_as(&Target { runtime_id: Some(vec![42, 9]), key: String::new(), ..token.clone() }));
}

/// The key carries the ancestors' segments: an editor rebuilt under the same parents is the same field, a same-type same-id control under another parent is not.
#[test]
fn the_key_tells_same_id_controls_under_different_parents_apart() {
    let token = Target {
        window: 10,
        pid: 4,
        thread: 5,
        runtime_id: Some(vec![42, 7]),
        key: "50004:editor/50026:notes/50032:root".into(),
        ..Target::default()
    };
    let rebuilt = Target { runtime_id: Some(vec![42, 9]), ..token.clone() };
    assert!(token.same_as(&rebuilt));
    let elsewhere = Target { runtime_id: Some(vec![42, 9]), key: "50004:editor/50026:title/50032:root".into(), ..token.clone() };
    assert!(!token.same_as(&elsewhere));
}

/// A token for a window with an element id matches only the same element; without one only the window level is enforced; window 0 matches anything.
#[test]
fn a_target_token_matches_the_same_place_only() {
    let token = Target { window: 10, class: "Edit".into(), pid: 4, thread: 5, runtime_id: Some(vec![42, 7]), ..Target::default() };
    assert!(token.same_as(&token.clone()));
    assert!(!token.same_as(&Target { runtime_id: Some(vec![42, 8]), ..token.clone() }));
    assert!(!token.same_as(&Target { runtime_id: None, ..token.clone() }));
    assert!(!token.same_as(&Target { window: 11, ..token.clone() }));
    assert!(!token.same_as(&Target { pid: 9, ..token.clone() }));
    assert!(!token.same_as(&Target { thread: 9, ..token.clone() }));
    let window_level = Target { runtime_id: None, ..token.clone() };
    assert!(window_level.same_as(&Target { runtime_id: Some(vec![1]), ..token.clone() }));
    assert!(Target::window_only(0).same_as(&token));
    assert!(Target::window_only(10).same_as(&token));
    assert!(!Target::window_only(10).same_as(&Target::default()));
}

/// A text that could not stay on the clipboard because the user copied something newer is reported as not saved, and says why; the newer copy is the reason, not an error of Cue's.
#[test]
fn a_newer_copy_is_named_as_the_reason_the_text_is_not_on_the_clipboard() {
    let receipt = Receipt { placement: Placement::Uncertain, failure: Some(Failure::NotRead), newer_copy: true, ..Receipt::nothing() };
    match receipt.into_result() {
        Err(InsertError::Failed { why, copied: false }) => assert!(why.contains("newer")),
        other => panic!("{other:?}"),
    }
    let elevated = Receipt { failure: Some(Failure::Elevated), ..Receipt::nothing() };
    assert!(matches!(elevated.into_result(), Err(InsertError::Failed { copied: false, .. })));
}
