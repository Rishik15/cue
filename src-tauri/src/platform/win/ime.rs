//! File: ime.rs
//! Purpose: Tell whether the focused app's input method (Japanese, Chinese, Korean...) would turn typed Unicode events into a composition, so `insert` pastes instead.
//! Contents: ImeState — Active, Inactive or Unknown; state — ask the focused window's IME; decide — the pure rule from the two answers.
//! An IME that does not answer in time is Unknown, and Unknown pastes: typing into a composition we cannot see would corrupt it. The answers describe the IME mode, not whether a composition is under way in a field; that is not observable across processes.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::Ime::{ImmGetDefaultIMEWnd, IME_CMODE_NATIVE};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SendMessageTimeoutW, GUITHREADINFO, SMTO_ABORTIFHUNG, WM_IME_CONTROL,
};

/// `WM_IME_CONTROL` command: read the conversion mode.
const IMC_GETCONVERSIONMODE: usize = 0x0001;
/// `WM_IME_CONTROL` command: read whether the IME is open.
const IMC_GETOPENSTATUS: usize = 0x0005;

/// What is known about the focused window's IME.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImeState {
    /// Open in a native conversion mode: typed characters would enter a composition.
    Active,
    /// Closed or in an alphanumeric mode, or the window has no IME.
    Inactive,
    /// The IME did not answer.
    Unknown,
}

/// The IME is another process's state, so `ImmGetContext` is not usable; the default IME window answers `WM_IME_CONTROL`. A hung target is given 100 ms. Safety: `answer` outlives the call.
fn ask(ime: HWND, command: usize) -> Option<usize> {
    let mut answer = 0usize;
    let sent = unsafe { SendMessageTimeoutW(ime, WM_IME_CONTROL, WPARAM(command), LPARAM(0), SMTO_ABORTIFHUNG, 100, Some(&mut answer)) };
    (sent.0 != 0).then_some(answer)
}

/// The state from the two answers (open status, conversion mode): a closed IME is Inactive whatever its mode; an unanswered question is Unknown unless the IME is known closed.
pub fn decide(open: Option<usize>, mode: Option<usize>) -> ImeState {
    match (open, mode) {
        (Some(0), _) => ImeState::Inactive,
        (Some(_), Some(mode)) if mode as u32 & IME_CMODE_NATIVE.0 != 0 => ImeState::Active,
        (Some(_), Some(_)) => ImeState::Inactive,
        _ => ImeState::Unknown,
    }
}

/// The state of the focused window's IME. Safety: window queries only; the `GUITHREADINFO` is a local with its size set.
pub fn state() -> ImeState {
    unsafe {
        let foreground = GetForegroundWindow();
        let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        let focus = match GetGUIThreadInfo(GetWindowThreadProcessId(foreground, None), &mut info) {
            Ok(()) if !info.hwndFocus.is_invalid() => info.hwndFocus,
            _ => foreground,
        };
        let ime = ImmGetDefaultIMEWnd(focus);
        if ime.is_invalid() {
            return ImeState::Inactive;
        }
        decide(ask(ime, IMC_GETOPENSTATUS), ask(ime, IMC_GETCONVERSIONMODE))
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, ImeState};

    /// A native mode on an open IME is active, a closed IME is inactive, and a missing answer is unknown (so the text is pasted).
    #[test]
    fn unanswered_ime_questions_are_unknown() {
        assert_eq!(decide(Some(1), Some(1)), ImeState::Active);
        assert_eq!(decide(Some(1), Some(0)), ImeState::Inactive);
        assert_eq!(decide(Some(0), None), ImeState::Inactive);
        assert_eq!(decide(None, None), ImeState::Unknown);
        assert_eq!(decide(Some(1), None), ImeState::Unknown);
    }
}
