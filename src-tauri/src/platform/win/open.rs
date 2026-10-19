//! Purpose: Open things the way Explorer would: a web address in the default browser, a program (an installer) with the Windows prompt it may need.
//! Contents: open_url — hands the address to the URL protocol handler, which uses the default browser; start_program — runs a program with arguments without waiting for it.
//! `explorer.exe <url>` opens a folder window for any address with a `?` query, so the handler is called directly. Callers pass only addresses and files of their own, never user input.
use std::path::Path;
use std::process::Command;

use windows::core::{w, HRESULT, HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_CANCELLED;
use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn open_url(url: &str) -> Result<(), String> {
    Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open the browser: {e}"))
}

/// Starts `path` with `args` and returns at once. Through the shell, so a program that wants administrator rights gets its normal prompt instead of failing to start.
/// Ok(false) when the user turned that prompt down; Err for anything else that stopped it.
pub fn start_program(path: &Path, args: &str) -> Result<bool, String> {
    let (file, params) = (HSTRING::from(path.as_os_str()), HSTRING::from(args));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` and the two strings it points to live until the call returns.
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) => Ok(true),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => Ok(false),
        Err(e) => Err(format!("Could not start the installer: {e}")),
    }
}
