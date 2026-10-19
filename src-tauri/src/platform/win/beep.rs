//! Purpose: The system's default beep, for a timer that finished (Windows).
//! Contents: beep — play it.

use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::UI::WindowsAndMessaging::MB_OK;

pub fn beep() {
    // SAFETY: a plain call; a machine without a sound device just stays silent.
    let _ = unsafe { MessageBeep(MB_OK) };
}
