//! File: platform/tests/delivery.rs
//! Purpose: Protect what the user is told when text could not be delivered: whether the text reached the app or sits on the clipboard, and the line on the pill for each outcome.
//! Contents: tests of `InsertError` and `InsertOptions`.

use crate::platform::{InsertError, InsertOptions, Submit};

/// Every way a delivery can end. Returns one of each kind (and both copied states where they matter).
fn outcomes() -> Vec<InsertError> {
    vec![
        InsertError::Elevated,
        InsertError::NotTaken,
        InsertError::FocusMoved { copied: true },
        InsertError::FocusMoved { copied: false },
        InsertError::Failed { why: "no focus".into(), copied: true },
        InsertError::Failed { why: "no focus".into(), copied: false },
        InsertError::NotRestored,
        InsertError::KeepCopy,
        InsertError::Submit("blocked".into()),
    ]
}

/// The pill and the text-went-in flag agree for every outcome: errors where the text went in say "Inserted", the others say it could not be inserted and whether it was copied.
#[test]
fn the_pill_says_where_the_text_went() {
    for error in outcomes() {
        let pill = error.pill();
        assert_eq!(error.text_went_in(), pill.starts_with("Inserted"), "{error:?}: {pill}");
        assert_eq!(
            pill.starts_with("Could not insert, copied"),
            matches!(
                error,
                InsertError::Elevated
                    | InsertError::NotTaken
                    | InsertError::FocusMoved { copied: true }
                    | InsertError::Failed { copied: true, .. }
            )
        );
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
    assert!(!options.always_paste && options.restore_clipboard && !options.keep_copy);
    assert_eq!((options.target, options.submit), (0, Submit::None));
}
