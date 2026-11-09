//! File: context.rs
//! Purpose: Ask the focused text field about itself through Windows UI Automation: the selected text and where it is on the screen, the few characters before the cursor (so a dictation can be fitted to them), whether the selection is empty and whether it is a password field.
//! Every question uses one `Session` (COM, one automation object with a short timeout, the focused element), so a flow pays for UI Automation once and a hung target cannot hold it for seconds. Only the text pattern of the focused element is asked for; the element tree is never walked.
//! Contents: focused_field / focused_field_in / field_of — the focused element's runtime id and rebuild-proof key (own segment plus its ancestors'); stayed — the rule that a read belongs to the field still focused after it; empty_verdict — the pure rule for a known-empty selection; Session — COM plus the automation object (released in that order), opened once per flow; uia_selection / selection_in — the selected text (capped) and its rectangle, rejected when empty or off-screen (visible_rect); field_state — password field, empty selection or neither; focused_is_password; text_before_cursor — up to `LOOKBACK` characters before the caret or the start of the selection.
//! All of them return None / false when the app does not expose its text (many do not), the field is a password field, or anything fails. Call from a worker thread: they talk to another process.

use windows::core::Interface;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, SAFEARRAY};
use windows::Win32::System::Ole::{SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomation2, IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationTextRange,
    IUIAutomationTextRangeArray, SupportedTextSelection_None, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, UIA_TextPatternId,
};

use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

use super::com::Com;
use crate::platform::{Rect, UiaSelection};

/// How many characters before the caret `text_before_cursor` reads.
const LOOKBACK: i32 = 3;
/// How long one question to another process may take, in ms.
const TIMEOUT_MS: u32 = 200;

/// Field order matters: the automation object is released before COM is uninitialised. Keep a session bound for as long as anything made from it lives.
pub struct Session {
    automation: IUIAutomation,
    _com: Com,
}

impl Session {
    /// Opens a session on this thread; None when UI Automation is not available. Safety: creates an automation object on this thread.
    pub fn open() -> Option<Session> {
        unsafe { Session::new() }
    }

    /// Safety: COM is initialised on this thread by the `Com` guard kept in the session.
    unsafe fn new() -> Option<Session> {
        let com = Com::init();
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        if let Ok(more) = automation.cast::<IUIAutomation2>() {
            let _ = more.SetConnectionTimeout(TIMEOUT_MS);
            let _ = more.SetTransactionTimeout(TIMEOUT_MS);
        }
        Some(Session { automation, _com: com })
    }

    /// The focused element, if any. Safety: the element lives no longer than the session.
    unsafe fn focused(&self) -> Option<IUIAutomationElement> {
        self.automation.GetFocusedElement().ok()
    }

    /// The first selected range of the focused element, when it has a text pattern and is not a password field. Safety: as `focused`.
    unsafe fn first_selection(&self) -> Option<IUIAutomationTextRange> {
        selection_range(&self.focused()?)
    }
}

/// The first selected range of `element`, when it has a text pattern, a selection and is not a password field. Safety: UI Automation calls on an element the caller owns.
unsafe fn selection_range(element: &IUIAutomationElement) -> Option<IUIAutomationTextRange> {
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

/// Up to `LOOKBACK` characters before the caret or the start of the selection (typing replaces a selection); `Some("")` is the start of the field. Safety: UI Automation calls on objects made here.
pub fn text_before_cursor() -> Option<String> {
    unsafe {
        let session = Session::new()?;
        let range = session.first_selection()?.Clone().ok()?;
        range.MoveEndpointByRange(TextPatternRangeEndpoint_End, &range, TextPatternRangeEndpoint_Start).ok()?;
        range.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -LOOKBACK).ok()?;
        Some(range.GetText(LOOKBACK).ok()?.to_string())
    }
}

/// Whether the focused field is a password field. False when the app does not say, so only a field that declares itself a password is skipped. Safety: UI Automation calls on objects made here.
pub fn focused_is_password() -> bool {
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

/// One look at the focused field. A selection is `Empty` only when the field's text pattern supports selection and says so with at least one range, every one of them collapsed (an insertion point); no pattern, no ranges, a failing provider and anything else are `Other`. Safety: UI Automation calls on objects that live no longer than the session.
pub fn field_state(session: Option<&Session>) -> FieldState {
    unsafe {
        let Some(element) = session.and_then(|s| s.focused()) else { return FieldState::Other };
        if element.CurrentIsPassword().is_ok_and(|p| p.as_bool()) {
            return FieldState::Password;
        }
        let Ok(pattern) = element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) else { return FieldState::Other };
        let supports = pattern.SupportedTextSelection().is_ok_and(|s| s != SupportedTextSelection_None);
        let Ok(ranges) = pattern.GetSelection() else { return FieldState::Other };
        let collapsed: Vec<Option<bool>> = (0..ranges.Length().unwrap_or(0)).map(|i| is_collapsed(&ranges, i)).collect();
        if empty_verdict(supports, &collapsed) {
            FieldState::Empty
        } else {
            FieldState::Other
        }
    }
}

/// Whether range `i` of the array is collapsed to an insertion point; None when the provider fails. Safety: UI Automation calls on objects the caller owns.
unsafe fn is_collapsed(ranges: &IUIAutomationTextRangeArray, i: i32) -> Option<bool> {
    let range = ranges.GetElement(i).ok()?;
    let order = range.CompareEndpoints(TextPatternRangeEndpoint_Start, &range, TextPatternRangeEndpoint_End).ok()?;
    Some(order == 0)
}

/// Whether the answers of a field's text pattern prove that nothing is selected: selection is supported, there is at least one range and every range is collapsed (`Some(true)`). A range that failed to answer (`None`) or a missing range proves nothing.
pub fn empty_verdict(supports_selection: bool, collapsed: &[Option<bool>]) -> bool {
    supports_selection && !collapsed.is_empty() && collapsed.iter().all(|c| *c == Some(true))
}

/// The focused element as far as the identity of a field goes.
pub struct Field {
    /// UI Automation's runtime id, which tells two fields of one window apart.
    pub runtime_id: Vec<i32>,
    /// Control type and automation id of the element and its ancestors, which survive an app rebuilding the element and tell same-id controls under different parents apart; empty when the element has no automation id.
    pub key: String,
}

/// The focused element's identity on a session of its own; None when the app exposes none or anything fails.
pub fn focused_field() -> Option<Field> {
    focused_field_in(&Session::open()?)
}

/// The focused element's identity on the caller's session. Safety: the SAFEARRAY is read while locked and destroyed before returning.
pub fn focused_field_in(session: &Session) -> Option<Field> {
    unsafe { field_of(session, &session.focused()?) }
}

/// How many ancestors of a field go into its key.
const PARENT_DEPTH: usize = 8;

/// The control type and automation id of `element` as one key segment; the id part is empty when the element has none, so an unnamed ancestor still tells its type. Safety: UI Automation calls on an element the caller owns.
unsafe fn segment(element: &IUIAutomationElement) -> String {
    let automation = element.CurrentAutomationId().map(|a| a.to_string()).unwrap_or_default();
    format!("{}:{automation}", element.CurrentControlType().map_or(0, |c| c.0))
}

/// The key of a field: its own segment and those of its ancestors up to `PARENT_DEPTH`, so two same-type same-id controls under different parents differ. Empty when the field itself has no automation id. Safety: UI Automation calls on objects the caller owns.
unsafe fn field_key(session: &Session, element: &IUIAutomationElement) -> String {
    let own = segment(element);
    if own.ends_with(':') {
        return String::new();
    }
    let Ok(walker) = session.automation.ControlViewWalker() else { return String::new() };
    let mut chain = vec![own];
    let mut current = element.clone();
    for _ in 0..PARENT_DEPTH {
        let Ok(parent) = walker.GetParentElement(&current) else { break };
        chain.push(segment(&parent));
        current = parent;
    }
    chain.join("/")
}

/// The identity of `element`. Safety: the SAFEARRAY is read while locked and destroyed before returning.
unsafe fn field_of(session: &Session, element: &IUIAutomationElement) -> Option<Field> {
    let array = element.GetRuntimeId().ok()?;
    if array.is_null() {
        return None;
    }
    let (low, high) = (SafeArrayGetLBound(array, 1).ok()?, SafeArrayGetUBound(array, 1).ok()?);
    let mut data = std::ptr::null_mut();
    let id = SafeArrayAccessData(array, &mut data).ok().map(|()| {
        let ids = std::slice::from_raw_parts(data as *const i32, (high - low + 1).max(0) as usize).to_vec();
        let _ = SafeArrayUnaccessData(array);
        ids
    });
    let _ = SafeArrayDestroy(array);
    let runtime_id = id.filter(|ids| !ids.is_empty())?;
    Some(Field { runtime_id, key: field_key(session, element) })
}

/// The selected text and where its first line is. At most `limit + 1` characters are read: a longer selection comes back one character over, which tells the caller it is too long without copying it. None when nothing is selected or the app does not expose its text.
pub fn uia_selection(limit: usize) -> Option<UiaSelection> {
    let session = Session::open()?;
    selection_in(&session, limit).map(|(found, _)| found)
}

/// Whether the field focused after a read is the one that was read: a focus that moved, or one whose identity cannot be read, proves nothing about where the text came from.
pub fn stayed(read: &Field, after: Option<&Field>) -> bool {
    after.is_some_and(|after| after.runtime_id == read.runtime_id)
}

/// `uia_selection` on a session the caller already has (one flow, one session), with the identity of the element the text was read from; None when the focus moved to another field during the read. A password field is never read. Safety: UI Automation calls on objects made from `session`; the rectangle array is destroyed after it is read.
pub fn selection_in(session: &Session, limit: usize) -> Option<(UiaSelection, Field)> {
    if field_state(Some(session)) == FieldState::Password {
        return None;
    }
    unsafe {
        let element = session.focused()?;
        let field = field_of(session, &element)?;
        let range = selection_range(&element)?;
        let text = range.GetText(limit as i32 + 1).ok()?.to_string();
        if text.is_empty() {
            return None;
        }
        let rect = range.GetBoundingRectangles().ok().and_then(|array| {
            let rect = first_rect(array);
            let _ = SafeArrayDestroy(array);
            rect
        });
        stayed(&field, focused_field_in(session).as_ref()).then_some((UiaSelection { text, rect }, field))
    }
}

/// The first rectangle of a UI Automation bounding-rectangle array: left, top, width, height as doubles. Safety: `array` is a valid SAFEARRAY of at least four doubles, locked here and unlocked before returning.
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

/// The bounds of the whole virtual screen. Safety: plain metric queries.
fn virtual_screen() -> Rect {
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
    use super::{empty_verdict, stayed, Field};

    /// A read belongs to the field only while the same element still has the focus after it.
    #[test]
    fn a_read_stays_with_its_field_only_if_the_focus_did_not_move() {
        let a = Field { runtime_id: vec![1, 2], key: String::new() };
        let b = Field { runtime_id: vec![1, 3], key: String::new() };
        assert!(stayed(&a, Some(&Field { runtime_id: vec![1, 2], key: "x".into() })));
        assert!(!stayed(&a, Some(&b)));
        assert!(!stayed(&a, None));
    }

    /// A selection is known empty only with support, at least one range and every range collapsed.
    #[test]
    fn only_collapsed_ranges_with_support_prove_an_empty_selection() {
        assert!(empty_verdict(true, &[Some(true)]));
        assert!(empty_verdict(true, &[Some(true), Some(true)]));
        assert!(!empty_verdict(true, &[]));
        assert!(!empty_verdict(false, &[Some(true)]));
        assert!(!empty_verdict(true, &[Some(true), Some(false)]));
        assert!(!empty_verdict(true, &[None]));
        assert!(!empty_verdict(true, &[Some(true), None]));
    }
}
