//! File: cleanup.rs
//! Purpose: Delete directories that are still locked while Cue runs (the WebView2 profile) once the process has exited.
//! Contents: remove_dirs_after_exit — starts a hidden shell that waits about two seconds (`ping`: `timeout` refuses to run without a console), then deletes each directory, trying again once a second for up to ten seconds while something (a worker still shutting down, a virus scanner) holds a file.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

/// Process creation flag: no console window for the shell.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Starts the hidden shell that deletes `dirs` once Cue has exited, and returns at once. Err with a line for the user when the shell cannot be started, so the caller can say the folders will stay. The command line is passed with `raw_arg`, because `args` would escape the inner quotes as `\"`, which cmd does not understand, so every rmdir would fail silently.
pub fn remove_dirs_after_exit(dirs: &[PathBuf]) -> Result<(), String> {
    let removals: Vec<String> = dirs
        .iter()
        .map(|d| {
            let d = d.display();
            format!("for /L %i in (1,1,10) do @if exist \"{d}\" (rmdir /S /Q \"{d}\" 2>nul & ping -n 2 127.0.0.1 >nul)")
        })
        .collect();
    let script = format!("ping -n 3 127.0.0.1 >nul & {}", removals.join(" & "));
    Command::new("cmd")
        .raw_arg(format!("/C \"{script}\""))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cue could not start the cleanup that removes its folders: {e}"))
}
