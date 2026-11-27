//! File: dictation/routing.rs
//! Purpose: Decides where a finished dictation's result goes and sends it there: only finished text is ever inserted (and so run through the rules and submitted); words kept by a failed engine or from a lossy recording go to the clipboard and a message, whatever the recording's length.
//! Contents: Route — the destinations; route — result to destination; Deliver — the three ways a result leaves; Live — the real delivery; dispatch — destination to delivery.

use tauri::AppHandle;

use super::{deliver, Spoken};
use crate::audio::Loss;
use crate::speech;
use crate::status;

/// Where a finished dictation's result goes.
#[derive(Debug, PartialEq)]
pub enum Route {
    /// An accidental tap: nothing was said.
    Nothing,
    /// A finished dictation: clean, route through the rules, insert or submit.
    Insert(String),
    /// The engine failed or the recording lost audio, after words were recognised: copy them to the clipboard, never insert or submit them.
    Rescue { message: String, kept: String },
    /// A failure with nothing recognised: only the message.
    Report(String),
}

/// Decides where a dictation's result goes. Inputs: what the speech engine returned, whether the recording was long enough to keep and what it lost. Returns the route; only `Ok` text of a lossless recording can be `Insert`, the words of a lossy one are rescued.
pub fn route(result: Result<String, speech::Failure>, keep: bool, loss: Option<Loss>) -> Route {
    match result {
        Ok(_) if !keep => Route::Nothing,
        Ok(text) if loss.is_some() && !text.trim().is_empty() => {
            let message = match loss {
                Some(Loss::Broken) => "The microphone stopped working.",
                Some(Loss::Failed) => "The microphone recording failed.",
                _ => "Part of the recording was lost.",
            };
            Route::Rescue { message: message.into(), kept: text }
        }
        Ok(text) => Route::Insert(text),
        Err(speech::Failure { message, kept: Some(kept) }) => Route::Rescue { message, kept },
        Err(failure) => Route::Report(failure.message),
    }
}

/// The ways a result leaves the dictation: insertion (cleaning, rules, typing, submit), the clipboard rescue and a plain message.
pub trait Deliver {
    /// Inserts finished text through the rules. Input: the text.
    fn insert(self, text: String);
    /// Copies kept words to the clipboard and tells the user why. Inputs: the failure message and the kept words.
    fn rescue(self, message: &str, kept: &str);
    /// Only tells the user. Input: the message.
    fn report(self, message: &str);
}

/// The real delivery of one dictation: its app and its window.
pub struct Live<'a> {
    pub app: &'a AppHandle,
    pub spoken: Spoken,
}

impl Deliver for Live<'_> {
    /// Hands the text to the delivery thread. Input: the text.
    fn insert(self, text: String) {
        deliver::insert_off_thread(self.app, text, self.spoken);
    }

    /// Copies the kept words to the clipboard. Inputs: the failure message and the kept words.
    fn rescue(self, message: &str, kept: &str) {
        deliver::rescue_partial(self.app, message, kept, self.spoken);
    }

    /// Shows the message. Input: the message.
    fn report(self, message: &str) {
        status::report(self.app, message);
        let mut spoken = self.spoken;
        spoken.log("reported");
    }
}

/// Sends a result where its route says. Inputs: the route and the delivery.
pub fn dispatch(routed: Route, out: impl Deliver) {
    match routed {
        Route::Nothing => {}
        Route::Insert(text) => out.insert(text),
        Route::Rescue { message, kept } => out.rescue(&message, &kept),
        Route::Report(message) => out.report(&message),
    }
}
