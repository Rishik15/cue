//! Purpose: Detect a focused app whose input method (Japanese, Chinese, Korean...) is converting keystrokes into native characters.
//! Contents: native_input_active — true when the focused window's IME is open in a native (non-alphanumeric) conversion mode.
//! Typed Unicode events would enter that IME's composition, so `insert` pastes instead.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::Ime::{ImmGetDefaultIMEWnd, IME_CMODE_NATIVE};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SendMessageTimeoutW, GUITHREADINFO, SMTO_ABORTIFHUNG, WM_IME_CONTROL,
};

const IMC_GETCONVERSIONMODE: usize = 0x0001;
const IMC_GETOPENSTATUS: usize = 0x0005;

/// The IME is another process's state, so `ImmGetContext` is not usable; the default IME window answers `WM_IME_CONTROL`.
/// A hung target is given 100 ms; every failure reads as "no IME", which keeps the typing path.
fn ask(ime: HWND, command: usize) -> Option<usize> {
    let mut answer = 0usize;
    let sent = unsafe { SendMessageTimeoutW(ime, WM_IME_CONTROL, WPARAM(command), LPARAM(0), SMTO_ABORTIFHUNG, 100, Some(&mut answer)) };
    (sent.0 != 0).then_some(answer)
}

pub fn native_input_active() -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        let focus = match GetGUIThreadInfo(GetWindowThreadProcessId(foreground, None), &mut info) {
            Ok(()) if !info.hwndFocus.is_invalid() => info.hwndFocus,
            _ => foreground,
        };
        let ime = ImmGetDefaultIMEWnd(focus);
        if ime.is_invalid() {
            return false;
        }
        ask(ime, IMC_GETOPENSTATUS).is_some_and(|open| open != 0)
            && ask(ime, IMC_GETCONVERSIONMODE).is_some_and(|mode| mode as u32 & IME_CMODE_NATIVE.0 != 0)
    }
}
