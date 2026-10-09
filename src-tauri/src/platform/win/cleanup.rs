//! Purpose: Delete directories that are still locked while Cue runs (the WebView2 profile) once the process has exited.
//! Contents: remove_dirs_after_exit — starts a hidden shell that waits two seconds, then deletes each directory.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn remove_dirs_after_exit(dirs: &[PathBuf]) {
    let removals: Vec<String> = dirs.iter().map(|d| format!("rmdir /S /Q \"{}\"", d.display())).collect();
    let script = format!("timeout /T 2 /NOBREAK >nul & {}", removals.join(" & "));
    let _ = Command::new("cmd").args(["/C", &script]).creation_flags(CREATE_NO_WINDOW).spawn();
}
