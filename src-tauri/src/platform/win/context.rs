//! Purpose: Ask the focused text field about itself (Windows UI Automation): the few characters before the cursor, so a dictation can be fitted to them, and whether it is a password field.
//! Contents: selection_is_empty — true when the field says nothing is selected; focused_is_password — true for a field that declares itself a password field; text_before_cursor — up to `LOOKBACK` characters before the caret or the start of the selection; None when the app does not
//! expose its text (many do not), the field is a password field, or anything fails.

use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, UIA_TextPatternId,
};

use super::com::Com;

const LOOKBACK: i32 = 3;

/// `Some("")` is the start of the field. Call from a worker thread: it talks to another process.
pub fn text_before_cursor() -> Option<String> {
    let _com = Com::init();
    unsafe {
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let element = automation.GetFocusedElement().ok()?;
        if element.CurrentIsPassword().ok()?.as_bool() {
            return None;
        }
        let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
        let selection = pattern.GetSelection().ok()?;
        if selection.Length().ok()? == 0 {
            return None;
        }
        let range = selection.GetElement(0).ok()?.Clone().ok()?;
        // Collapse to the start of the selection (typing replaces a selection), then look back.
        range.MoveEndpointByRange(TextPatternRangeEndpoint_End, &range, TextPatternRangeEndpoint_Start).ok()?;
        range.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -LOOKBACK).ok()?;
        Some(range.GetText(LOOKBACK).ok()?.to_string())
    }
}

/// Whether the focused field is a password field. False when the app does not say, so only a field that declares itself a password is skipped. Call from a worker thread.
pub fn focused_is_password() -> bool {
    let _com = Com::init();
    // SAFETY: UI Automation calls on objects made here.
    unsafe {
        let automation: Option<IUIAutomation> = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok();
        let element = automation.and_then(|a| a.GetFocusedElement().ok());
        element.and_then(|e| e.CurrentIsPassword().ok()).is_some_and(|p| p.as_bool())
    }
}

/// Whether the focused field reports an empty selection. False when it reports a selection or does not expose its text (most apps do not, and some report wrongly), so this only speeds up the answer, never decides it.
/// Call from a worker thread.
pub fn selection_is_empty() -> bool {
    let _com = Com::init();
    // SAFETY: UI Automation calls on objects made here.
    unsafe {
        let automation: Option<IUIAutomation> = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok();
        let selection = automation
            .and_then(|a| a.GetFocusedElement().ok())
            .and_then(|e| e.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId).ok())
            .and_then(|p| p.GetSelection().ok());
        let Some(ranges) = selection else { return false };
        let length = ranges.Length().unwrap_or(1);
        length == 0 || (length == 1 && ranges.GetElement(0).and_then(|r| r.GetText(1)).is_ok_and(|t| t.is_empty()))
    }
}
