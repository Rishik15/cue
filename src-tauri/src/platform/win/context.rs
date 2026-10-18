//! Purpose: Ask the focused text field about itself through Windows UI Automation: the selected text and where it is on the screen, the few characters before the cursor (so a dictation can be fitted to them), whether the selection is empty and whether it is a password field.
//! Every question uses one `Session` (COM, one automation object with a short timeout, the focused element), so a flow pays for UI Automation once and a hung target cannot hold it for seconds. Only the text pattern of the focused element is asked for; the element tree is never walked.
//! Contents: Session — COM plus the automation object (released in that order), opened once per flow; uia_selection / selection_in — the selected text (capped) and its rectangle, rejected when empty or off-screen (visible_rect); field_state — password field, empty selection or neither; focused_is_password; text_before_cursor — up to `LOOKBACK` characters before the caret or the start of the selection.
//! All of them return None / false when the app does not expose its text (many do not), the field is a password field, or anything fails. Call from a worker thread: they talk to another process.

use windows::core::Interface;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, SAFEARRAY};
use windows::Win32::System::Ole::{SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomation2, IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationTextRange,
    TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, TextUnit_Character, UIA_TextPatternId,
};

use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

use super::com::Com;
use crate::platform::{Rect, UiaSelection};

const LOOKBACK: i32 = 3;
/// How long one question to another process may take, in ms.
const TIMEOUT_MS: u32 = 200;

/// Field order matters: the automation object is released before COM is uninitialised. Keep a session bound for as long as anything made from it lives.
pub struct Session {
    automation: IUIAutomation,
    _com: Com,
}

impl Session {
    pub fn open() -> Option<Session> {
        // SAFETY: creates an automation object on this thread.
        unsafe { Session::new() }
    }

    unsafe fn new() -> Option<Session> {
        let com = Com::init();
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        if let Ok(more) = automation.cast::<IUIAutomation2>() {
            let _ = more.SetConnectionTimeout(TIMEOUT_MS);
            let _ = more.SetTransactionTimeout(TIMEOUT_MS);
        }
        Some(Session { automation, _com: com })
    }

    unsafe fn focused(&self) -> Option<IUIAutomationElement> {
        self.automation.GetFocusedElement().ok()
    }

    /// The first selected range of the focused element, when it has a text pattern and is not a password field.
    unsafe fn first_selection(&self) -> Option<IUIAutomationTextRange> {
        let element = self.focused()?;
        if element.CurrentIsPassword().ok()?.as_bool() {
            return None;
        }
        let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
        let selection = pattern.GetSelection().ok()?;
        if selection.Length().ok()? == 0 {
            return None;
        }
        selection.GetElement(0).ok()
    }
}

/// `Some("")` is the start of the field.
pub fn text_before_cursor() -> Option<String> {
    // SAFETY: UI Automation calls on objects made here.
    unsafe {
        let session = Session::new()?;
        let range = session.first_selection()?.Clone().ok()?;
        // Collapse to the start of the selection (typing replaces a selection), then look back.
        range.MoveEndpointByRange(TextPatternRangeEndpoint_End, &range, TextPatternRangeEndpoint_Start).ok()?;
        range.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -LOOKBACK).ok()?;
        Some(range.GetText(LOOKBACK).ok()?.to_string())
    }
}

/// Whether the focused field is a password field. False when the app does not say, so only a field that declares itself a password is skipped.
pub fn focused_is_password() -> bool {
    // SAFETY: UI Automation calls on objects made here.
    unsafe {
        let session = Session::new();
        let element = session.as_ref().and_then(|s| s.focused());
        element.and_then(|e| e.CurrentIsPassword().ok()).is_some_and(|p| p.as_bool())
    }
}

/// What one look at the focused field says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldState {
    /// It declares itself a password field: Cue never reads from it.
    Password,
    /// It reports an empty selection (so a copy would find nothing).
    Empty,
    /// Anything else, including an app that does not expose its text (most do not, and some report wrongly): this only speeds up an answer, never decides one.
    Other,
}

pub fn field_state(session: Option<&Session>) -> FieldState {
    // SAFETY: UI Automation calls on objects that live no longer than the session.
    unsafe {
        let Some(element) = session.and_then(|s| s.focused()) else { return FieldState::Other };
        if element.CurrentIsPassword().is_ok_and(|p| p.as_bool()) {
            return FieldState::Password;
        }
        let selection = element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId).ok().and_then(|p| p.GetSelection().ok());
        let Some(ranges) = selection else { return FieldState::Other };
        let length = ranges.Length().unwrap_or(1);
        let empty = length == 0 || (length == 1 && ranges.GetElement(0).and_then(|r| r.GetText(1)).is_ok_and(|t| t.is_empty()));
        if empty {
            FieldState::Empty
        } else {
            FieldState::Other
        }
    }
}

/// The selected text and where its first line is. At most `limit + 1` characters are read: a longer selection comes back one character over, which tells the caller it is too long without copying it. None when nothing is selected or the app does not expose its text.
pub fn uia_selection(limit: usize) -> Option<UiaSelection> {
    let session = Session::open()?;
    selection_in(&session, limit)
}

/// `uia_selection` on a session the caller already has (one flow, one session). A password field is never read.
pub fn selection_in(session: &Session, limit: usize) -> Option<UiaSelection> {
    if field_state(Some(session)) == FieldState::Password {
        return None;
    }
    // SAFETY: UI Automation calls on objects made from `session`; the rectangle array is destroyed after it is read.
    unsafe {
        let range = session.first_selection()?;
        let text = range.GetText(limit as i32 + 1).ok()?.to_string();
        if text.is_empty() {
            return None;
        }
        let rect = range.GetBoundingRectangles().ok().and_then(|array| {
            let rect = first_rect(array);
            let _ = SafeArrayDestroy(array);
            rect
        });
        Some(UiaSelection { text, rect })
    }
}

/// The first rectangle of a UI Automation bounding-rectangle array: left, top, width, height as doubles.
unsafe fn first_rect(array: *const SAFEARRAY) -> Option<Rect> {
    if SafeArrayGetUBound(array, 1).ok()? - SafeArrayGetLBound(array, 1).ok()? < 3 {
        return None;
    }
    let mut data = std::ptr::null_mut();
    SafeArrayAccessData(array, &mut data).ok()?;
    let v = std::slice::from_raw_parts(data as *const f64, 4);
    let rect = visible_rect([v[0], v[1], v[2], v[3]], virtual_screen());
    let _ = SafeArrayUnaccessData(array);
    rect
}

fn virtual_screen() -> Rect {
    // SAFETY: plain metric queries.
    let (x, y, w, h) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    Rect { left: x, top: y, right: x + w, bottom: y + h }
}

/// The rectangle when it has a size and touches the screen. Apps report empty or far off-screen rectangles for hidden text, and a card placed by one would land nowhere.
fn visible_rect(v: [f64; 4], screen: Rect) -> Option<Rect> {
    let [left, top, width, height] = v;
    if !(width >= 1.0 && height >= 1.0 && left.is_finite() && top.is_finite()) {
        return None;
    }
    let rect = Rect { left: left as i32, top: top as i32, right: (left + width) as i32, bottom: (top + height) as i32 };
    let touches = rect.left < screen.right && rect.right > screen.left && rect.top < screen.bottom && rect.bottom > screen.top;
    touches.then_some(rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { left: 0, top: 0, right: 1920, bottom: 1080 };

    #[test]
    fn empty_or_off_screen_rectangles_are_rejected() {
        assert!(visible_rect([10.0, 10.0, 50.0, 20.0], SCREEN).is_some());
        assert!(visible_rect([10.0, 10.0, 0.0, 20.0], SCREEN).is_none());
        assert!(visible_rect([-32000.0, -32000.0, 50.0, 20.0], SCREEN).is_none());
        assert!(visible_rect([f64::NAN, 10.0, 50.0, 20.0], SCREEN).is_none());
        assert!(visible_rect([1900.0, 1070.0, 100.0, 100.0], SCREEN).is_some(), "partly visible still counts");
    }
}
