//! File: delivery.rs
//! Purpose: The platform-neutral vocabulary of text delivery, shared by the Windows backend and the stub so both have one surface: where the text is going (Target), how it is delivered, what follows it, and an honest receipt of what happened.
//! Contents: Capture — what reading a selection found (text, empty, unknown or refused); Target — an opaque token for the window and field the text is for, with `same_as` (window, process and thread equal and, when the token carries a UIA runtime id, the same field: that id, or the same control type and automation id after an app rebuilt the element); InsertOptions — how one delivery goes; Submit — the key pressed after the text; Receipt / Placement / Caret / ClipboardEnd — what happened, one fact each; InsertError — the receipt as the pill and status line need it.
//! A sent key is not a placed text: `Placement::Sent` means the input was injected, never that the app accepted it (only a UIA read-back could say so, and none is done), so nothing here is called confirmed.

use std::fmt;

/// The window and field a text is for, taken before anything slow happens. Without a UIA runtime id only the window level is enforced; that limit is deliberate (many apps expose no runtime id).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Target {
    /// The top-level window; 0 for any window.
    pub window: isize,
    /// The window class name.
    pub class: String,
    /// The process id.
    pub pid: u32,
    /// The thread id that owns the window.
    pub thread: u32,
    /// UI Automation's runtime id of the focused element, when it could be read.
    pub runtime_id: Option<Vec<i32>>,
    /// What stays the same when an app rebuilds the focused element (its control type and automation id); empty when the element has no automation id.
    pub key: String,
}

impl Target {
    /// A target that names only a window (the caller has nothing finer): any state of it matches.
    pub fn window_only(window: isize) -> Target {
        Target { window, ..Target::default() }
    }

    /// Whether `now` is still the place this token was taken at: the same window, process and thread, and the same field when this token has a runtime id (see `same_field`). A token with a runtime id never matches a `now` without one. A token for window 0 matches anything. Capture and delivery both use this one rule.
    pub fn same_as(&self, now: &Target) -> bool {
        if self.window == 0 {
            return true;
        }
        let window_level =
            self.window == now.window && (self.pid == 0 || self.pid == now.pid) && (self.thread == 0 || self.thread == now.thread);
        window_level && self.same_field(now)
    }

    /// Whether the focused element of `now` is this token's field: the same runtime id, or, when an app rebuilt the element, the same non-empty key (control type and automation id). A token without a runtime id names no field.
    fn same_field(&self, now: &Target) -> bool {
        let Some(id) = self.runtime_id.as_ref() else { return true };
        now.runtime_id.as_ref() == Some(id) || (!self.key.is_empty() && self.key == now.key && now.runtime_id.is_some())
    }
}

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
    /// The finer token (window, process, thread, focused element) taken earlier, when the caller has one; it replaces `target`.
    pub token: Option<Target>,
    /// Fits the text to the few characters before the cursor, read under the delivery lock from the field the text lands in (dictation: capitalisation and spacing); None leaves the text as it is.
    pub fit: Option<fn(&str, &str) -> String>,
    /// Type one character at a time with a pause (remote desktops, virtual machines that drop fast input); single-line text only.
    pub slow: bool,
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
        InsertOptions {
            always_paste: false,
            restore_clipboard: true,
            target: 0,
            token: None,
            fit: None,
            slow: false,
            keep_copy: false,
            submit: Submit::None,
            separator: "",
        }
    }
}

impl InsertOptions {
    /// The place the text is for: the token when there is one, else the window alone.
    pub fn place(&self) -> Target {
        self.token.clone().unwrap_or_else(|| Target::window_only(self.target))
    }
}

/// Whether the text went where it was sent. Never "confirmed": no app acknowledges input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The input was injected (and a paste was read by the target, when it was a paste).
    Sent,
    /// Some of the input may have gone in, or nobody asked for a pasted text: do not send it again by itself.
    Uncertain,
    /// Nothing was sent.
    NotDone,
}

/// What happened to the caret move after a paste or typed text with a `{cursor}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caret {
    /// No move was asked for.
    NotNeeded,
    /// The caret was moved.
    Moved,
    /// The move was skipped or failed, for this reason: a move after the user typed or switched windows would land in the wrong place.
    NotDone(String),
}

/// What became of the clipboard the user had.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardEnd {
    /// Never borrowed.
    Untouched,
    /// The old clipboard is back.
    Restored,
    /// It was left on purpose: the user copied something newer, or a clipboard agent read the text first (ADR 0014).
    Kept,
    /// It could not be put back or was never borrowed, for this reason.
    LeftBehind(String),
}

/// Everything a delivery did, one fact each.
#[derive(Clone, Debug)]
pub struct Receipt {
    pub placement: Placement,
    pub caret: Caret,
    pub clipboard: ClipboardEnd,
    /// The text is on the clipboard for the user to paste by hand (a rescue, or `keep_copy`).
    pub copied: bool,
    /// Somebody else copied something while the text waited on the clipboard, so the text is not on it and the newer copy was left alone.
    pub newer_copy: bool,
    /// The clipboard sequence number as Cue last left it (after its own publication), so a rescue copy can tell a newer copy by someone else; None until Cue wrote the clipboard.
    pub clipboard_seq: Option<u32>,
    /// The target itself asked for the pasted text (a paste only).
    pub target_read: bool,
    /// Why nothing, or not everything, was sent.
    pub failure: Option<Failure>,
    /// The submit key failed, for this reason.
    pub submit: Option<String>,
    /// Copying the text afterwards (`keep_copy`) failed.
    pub keep_copy_failed: bool,
}

/// Why a delivery did not send everything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The app runs as administrator; Windows drops Cue's input.
    Elevated,
    /// Focus or the field is not where the text was meant for.
    FocusMoved,
    /// Nobody had asked for the pasted text when the wait ended. The app may still take it (a slow app processes the keys late), so the text is neither reported as lost nor sent again.
    NotRead,
    /// Another Cue operation kept the coordinator for 5 s.
    Busy,
    /// Something failed for this reason.
    Other(String),
}

impl Receipt {
    /// A receipt of a delivery that has done nothing yet.
    pub fn nothing() -> Receipt {
        Receipt {
            placement: Placement::NotDone,
            caret: Caret::NotNeeded,
            clipboard: ClipboardEnd::Untouched,
            copied: false,
            newer_copy: false,
            clipboard_seq: None,
            target_read: false,
            failure: None,
            submit: None,
            keep_copy_failed: false,
        }
    }

    /// The receipt as the error the pill and status line report; None when everything asked for happened.
    pub fn into_result(self) -> Result<(), InsertError> {
        if let Some(failure) = &self.failure {
            let copied = self.copied;
            if self.newer_copy {
                let why = "you copied something newer, so Cue left your clipboard alone".to_string();
                return Err(InsertError::Failed { why, copied: false });
            }
            return Err(match failure {
                Failure::Elevated if copied => InsertError::Elevated,
                Failure::NotRead => {
                    InsertError::Uncertain { why: "the app had not asked for the pasted text yet, so it may still appear".into(), copied }
                }
                Failure::Elevated => InsertError::Failed { why: "the app runs as administrator".into(), copied },
                Failure::FocusMoved => InsertError::FocusMoved { copied },
                Failure::Busy => InsertError::Failed { why: "Cue is busy with another clipboard operation".into(), copied },
                Failure::Other(why) if self.placement == Placement::Uncertain => InsertError::Uncertain { why: why.clone(), copied },
                Failure::Other(why) => InsertError::Failed { why: why.clone(), copied },
            });
        }
        if let ClipboardEnd::LeftBehind(_) = self.clipboard {
            return Err(InsertError::NotRestored);
        }
        if self.keep_copy_failed {
            return Err(InsertError::KeepCopy);
        }
        if let Some(why) = self.submit {
            return Err(InsertError::Submit(why));
        }
        match self.caret {
            Caret::NotDone(why) => Err(InsertError::Caret(why)),
            _ => Ok(()),
        }
    }
}

/// How a delivery ended when it did not fully succeed. `copied` says whether the text was left on the clipboard instead.
#[derive(Debug)]
pub enum InsertError {
    /// The focused app runs as administrator; Windows blocks our input. The text was copied for a manual paste.
    Elevated,
    /// Focus was not in the target window (or field) at the moment of delivery, so nothing was typed.
    FocusMoved { copied: bool },
    /// Inserting failed for `why`.
    Failed { why: String, copied: bool },
    /// Part of the input may have gone in (`why` says what failed): it is not retried by itself.
    Uncertain { why: String, copied: bool },
    /// The text went in, but the clipboard the user had before could not be put back.
    NotRestored,
    /// The text went in, but copying it to the clipboard afterwards failed.
    KeepCopy,
    /// The text went in, but the submit key could not be pressed.
    Submit(String),
    /// The text went in, but the caret was not moved to its place.
    Caret(String),
}

impl InsertError {
    /// Whether the text reached the app even though this error is reported.
    pub fn text_went_in(&self) -> bool {
        matches!(self, InsertError::NotRestored | InsertError::KeepCopy | InsertError::Submit(_) | InsertError::Caret(_))
    }

    /// Whether the text was left on the clipboard instead of reaching the app.
    pub fn copied(&self) -> bool {
        matches!(
            self,
            InsertError::Elevated
                | InsertError::FocusMoved { copied: true }
                | InsertError::Uncertain { copied: true, .. }
                | InsertError::Failed { copied: true, .. }
        )
    }

    /// The short line for the pill on screen: it says where the text went.
    pub fn pill(&self) -> &'static str {
        match self {
            InsertError::Elevated
            | InsertError::FocusMoved { copied: true }
            | InsertError::Uncertain { copied: true, .. }
            | InsertError::Failed { copied: true, .. } => "Could not insert, copied to clipboard",
            InsertError::FocusMoved { .. } | InsertError::Failed { .. } | InsertError::Uncertain { .. } => "Could not insert the text",
            InsertError::NotRestored => "Inserted, but your old clipboard was not restored",
            InsertError::KeepCopy => "Inserted, but not copied to clipboard",
            InsertError::Submit(_) => "Inserted, but the submit key failed",
            InsertError::Caret(_) => "Inserted, but the cursor was not moved",
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
            InsertError::FocusMoved { copied: true } => {
                f.write_str("You switched windows, so the text was copied to the clipboard. Paste it with Ctrl+V.")
            }
            InsertError::FocusMoved { copied: false } => f.write_str("You switched windows and the text could not be copied."),
            InsertError::Failed { why, copied: true } => write!(f, "Could not insert text: {why}, so the text is on your clipboard"),
            InsertError::Failed { why, copied: false } => {
                write!(f, "Could not insert text: {why}, and the text could not be saved to the clipboard")
            }
            InsertError::Uncertain { why, copied: true } => {
                write!(f, "The text may or may not have been inserted ({why}). Look at the field before pasting; the whole text is on your clipboard.")
            }
            InsertError::Uncertain { why, copied: false } => {
                write!(f, "The text may or may not have been inserted ({why}), and it could not be saved to the clipboard")
            }
            InsertError::NotRestored => f.write_str("The text was inserted, but Cue could not put your previous clipboard back."),
            InsertError::KeepCopy => f.write_str("The text was inserted but could not be copied to the clipboard."),
            InsertError::Submit(why) => write!(f, "The text was inserted, but Cue could not press the submit key: {why}"),
            InsertError::Caret(why) => write!(f, "The text was inserted, but Cue did not move the cursor: {why}"),
        }
    }
}

impl From<String> for InsertError {
    /// A plain failure message becomes a failed delivery; the caller learns afterwards whether the text could be kept.
    fn from(why: String) -> Self {
        InsertError::Failed { why, copied: false }
    }
}

/// What reading the selection of the target found. Nothing is guessed: a copy that cannot be tied to the target is `Unknown`, and what is `Unknown` is neither used nor restored over.
pub enum Capture {
    /// The selected text (and its HTML), with whether the clipboard the user had was put back.
    Text(super::Copied),
    /// The field says nothing is selected, or the copy held no text.
    Empty,
    /// The target did not answer in time, or the clipboard changed in a way that is not provably its copy.
    Unknown,
    /// The flow was cancelled (Escape) before the copy was used: nothing is shown.
    Cancelled,
    /// Nothing was read, for this user-readable reason (password field, administrator app, busy, too long).
    Refused(String),
}
