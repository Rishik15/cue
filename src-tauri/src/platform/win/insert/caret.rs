//! File: insert/caret.rs
//! Purpose: The caret move after inserted text: it goes out only when the target is still in front and the user typed no key since the text was sent, and the result is the caret's own, never a failure of the text.
//! Contents: caret_still_safe — the one check; move_caret / move_caret_now — move the caret left after a settle pause or at once.
use windows::Win32::UI::Input::KeyboardAndMouse::VK_LEFT;

use super::{taps, try_send};
use crate::input::Mark;
use crate::platform::{Caret, Target};

/// Whether a caret move may go out now: the target is still in front and the user typed no key since `mark`. The one place that decides it.
fn caret_still_safe(want: &Target, mark: Mark) -> Result<(), &'static str> {
    if super::still_there(want).is_err() {
        return Err("focus moved to another window");
    }
    if mark.physical_keys_since() {
        return Err("a key was typed before the caret move");
    }
    Ok(())
}

/// Moves the caret `back` characters left once the paste has had time to land, unless it is no longer safe.
pub(super) fn move_caret(want: &Target, mark: Mark, back: usize) -> Caret {
    std::thread::sleep(super::SETTLE);
    move_caret_now(want, mark, back)
}

/// Moves the caret `back` characters left at once, unless it is no longer safe; the result is the caret's own, never a failure of the text.
pub(super) fn move_caret_now(want: &Target, mark: Mark, back: usize) -> Caret {
    if let Err(why) = caret_still_safe(want, mark) {
        return Caret::NotDone(why.into());
    }
    match try_send(&taps(VK_LEFT, back)) {
        Ok(()) => Caret::Moved,
        Err(short) => Caret::NotDone(short.to_string()),
    }
}
