//! File: delivery.rs
//! Purpose: The platform-neutral vocabulary of text delivery, shared by the Windows backend and the stub so both have one surface: how text is delivered, what follows it, and how a delivery can end.
//! Contents: InsertOptions — how one delivery goes (paste or type, restore, target window, copy afterwards, submit key); Submit — the key pressed after the text; InsertError — the structured outcome of a delivery that did not fully succeed, with the pill line and whether the text went in.

use std::fmt;

/// The key pressed once, after the text went in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Submit {
    #[default]
    None,
    Enter,
    CtrlEnter,
}

/// How one delivery goes; the defaults are what most people want.
pub struct InsertOptions {
    /// Skip typing and always paste, even for short single-line text.
    pub always_paste: bool,
    /// Put the previous clipboard back after a paste.
    pub restore_clipboard: bool,
    /// The window the text is for (0 for any): when focus is elsewhere at the moment of delivery, nothing is typed and the text goes to the clipboard.
    pub target: isize,
    /// Leave the inserted text on the clipboard once the insertion (and any restore) is over.
    pub keep_copy: bool,
    /// The key pressed after the text went in.
    pub submit: Submit,
    /// Typed or pasted after the text, and not part of what `keep_copy` leaves on the clipboard (dictation adds a space so the next words do not run into it).
    pub separator: &'static str,
}

impl Default for InsertOptions {
    /// Typing or pasting as fits the text, the old clipboard restored, no target check, no copy, no submit key.
    fn default() -> Self {
        InsertOptions { always_paste: false, restore_clipboard: true, target: 0, keep_copy: false, submit: Submit::None, separator: "" }
    }
}

/// How a delivery ended when it did not fully succeed. `copied` says whether the text was left on the clipboard instead.
#[derive(Debug)]
pub enum InsertError {
    /// The focused app runs as administrator; Windows blocks our input. The text was copied for a manual paste.
    Elevated,
    /// Nobody asked for the pasted text, so it was left on the clipboard.
    NotTaken,
    /// Focus was not in the target window at the moment of delivery, so nothing was typed.
    FocusMoved { copied: bool },
    /// Inserting failed for `why`.
    Failed { why: String, copied: bool },
    /// The text went in, but the clipboard the user had before could not be put back.
    NotRestored,
    /// The text went in, but copying it to the clipboard afterwards failed.
    KeepCopy,
    /// The text went in, but the submit key could not be pressed.
    Submit(String),
}

impl InsertError {
    /// Whether the text reached the app even though this error is reported.
    pub fn text_went_in(&self) -> bool {
        matches!(self, InsertError::NotRestored | InsertError::KeepCopy | InsertError::Submit(_))
    }

    /// The short line for the pill on screen: it says where the text went.
    pub fn pill(&self) -> &'static str {
        match self {
            InsertError::Elevated
            | InsertError::NotTaken
            | InsertError::FocusMoved { copied: true }
            | InsertError::Failed { copied: true, .. } => "Could not insert, copied to clipboard",
            InsertError::FocusMoved { .. } | InsertError::Failed { .. } => "Could not insert the text",
            InsertError::NotRestored => "Inserted, but your old clipboard was not restored",
            InsertError::KeepCopy => "Inserted, but not copied to clipboard",
            InsertError::Submit(_) => "Inserted, but the submit key failed",
        }
    }
}

impl fmt::Display for InsertError {
    /// The full sentence for the status line.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InsertError::Elevated => {
                f.write_str("This app runs as administrator, so Cue cannot type into it. The text is on your clipboard.")
            }
            InsertError::NotTaken => f.write_str("The app did not take the pasted text, so it was left on your clipboard."),
            InsertError::FocusMoved { copied: true } => {
                f.write_str("You switched windows, so the text was copied to the clipboard. Paste it with Ctrl+V.")
            }
            InsertError::FocusMoved { copied: false } => f.write_str("You switched windows and the text could not be copied."),
            InsertError::Failed { why, copied: true } => write!(f, "Could not insert text: {why}, so the text is on your clipboard"),
            InsertError::Failed { why, copied: false } => {
                write!(f, "Could not insert text: {why}, and the text could not be saved to the clipboard")
            }
            InsertError::NotRestored => f.write_str("The text was inserted, but Cue could not put your previous clipboard back."),
            InsertError::KeepCopy => f.write_str("The text was inserted but could not be copied to the clipboard."),
            InsertError::Submit(why) => write!(f, "The text was inserted, but Cue could not press the submit key: {why}"),
        }
    }
}

impl From<String> for InsertError {
    /// A plain failure message becomes a failed delivery; the caller learns afterwards whether the text could be kept.
    fn from(why: String) -> Self {
        InsertError::Failed { why, copied: false }
    }
}
