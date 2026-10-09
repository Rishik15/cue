//! Purpose: Detect a focused app that Windows will not let Cue type into (UIPI: input cannot go to a higher integrity level).
//! Contents: target_blocks_input — true when the foreground process is elevated and Cue is not.

use std::mem::size_of;
use std::sync::OnceLock;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// None when the token cannot be read, which for another process means it outranks us.
fn is_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut info = TOKEN_ELEVATION::default();
        let mut len = 0;
        let read =
            GetTokenInformation(token, TokenElevation, Some(&mut info as *mut _ as *mut _), size_of::<TOKEN_ELEVATION>() as u32, &mut len);
        let _ = CloseHandle(token);
        read.ok()?;
        Some(info.TokenIsElevated != 0)
    }
}

fn we_are_elevated() -> bool {
    static ELEVATED: OnceLock<bool> = OnceLock::new();
    *ELEVATED.get_or_init(|| is_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false))
}

pub fn target_blocks_input() -> bool {
    if we_are_elevated() {
        return false;
    }
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
        if pid == 0 {
            return false;
        }
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return true };
        let blocked = is_elevated(process).unwrap_or(true);
        let _ = CloseHandle(process);
        blocked
    }
}
