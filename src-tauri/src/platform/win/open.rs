//! Purpose: Open a web address in the default browser, and start a downloaded per-user installer silently.
//! Contents: open_url — hands the address to the URL protocol handler, which uses the default browser; run_installer — starts the installer directly (`CreateProcessW`, not the shell), so an unsigned per-user installer runs with no SmartScreen or "Open File" prompt and no window; it returns at once.
//! `explorer.exe <url>` opens a folder window for any address with a `?` query, so the handler is called directly. The installer path is Cue's own verified download, never user input. `CreateProcessW` is used on purpose: `ShellExecuteEx` would run the shell Attachment Execution Service (SmartScreen, the "Open File — Security Warning") on an unsigned exe, which is the prompt Cue is avoiding; a `currentUser` NSIS installer is `asInvoker`, so it needs no elevation.
use std::mem::size_of;
use std::path::Path;
use std::process::Command;

use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_ELEVATION_REQUIRED};
use windows::Win32::System::Threading::{CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW};

pub fn open_url(url: &str) -> Result<(), String> {
    Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open the browser: {e}"))
}

/// Starts the installer at `path` with `args` and returns at once. Direct, not through the shell, so an unsigned per-user installer runs with no security prompt and no window (NSIS `/S`).
/// Ok(true) when it started; Ok(false) when Windows wants administrator rights (an all-users install detected as per-user) so the caller can send the user to the download page; Err for anything else.
pub fn run_installer(path: &Path, args: &str) -> Result<bool, String> {
    let app = HSTRING::from(path.as_os_str());
    // The command line must start with the program, be writable and NUL-terminated (CreateProcessW may modify it).
    let mut cmd: Vec<u16> = format!("\"{}\" {args}", path.display()).encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = PROCESS_INFORMATION::default();
    let startup = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, ..Default::default() };
    // SAFETY: `app` and `cmd` outlive the call; `cmd` is writable and NUL-terminated.
    let result = unsafe {
        CreateProcessW(
            PCWSTR(app.as_ptr()),
            Some(PWSTR(cmd.as_mut_ptr())),
            None,
            None,
            false,
            PROCESS_CREATION_FLAGS(0),
            None,
            None,
            &startup,
            &mut info,
        )
    };
    match result {
        Ok(()) => {
            // SAFETY: both handles came from the successful call above.
            unsafe {
                let _ = CloseHandle(info.hThread);
                let _ = CloseHandle(info.hProcess);
            }
            Ok(true)
        }
        Err(e) if e.code() == ERROR_ELEVATION_REQUIRED.to_hresult() => Ok(false),
        Err(e) => Err(format!("Could not start the installer: {e}")),
    }
}
