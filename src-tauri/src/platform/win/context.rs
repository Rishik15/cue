//! Purpose: Read the few characters before the cursor in the focused text field (Windows UI Automation), so a dictation can be fitted to them.
//! Contents: text_before_cursor — up to `LOOKBACK` characters before the caret or the start of the selection; None when the app does not
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
