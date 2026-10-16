//! Purpose: Delete directories that are still locked while Cue runs (the WebView2 profile) once the process has exited.
//! Contents: remove_dirs_after_exit — starts a hidden shell that waits about two seconds (`ping`: `timeout` refuses to run without a console), then deletes each directory.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn remove_dirs_after_exit(dirs: &[PathBuf]) {
    let removals: Vec<String> = dirs.iter().map(|d| format!("rmdir /S /Q \"{}\"", d.display())).collect();
    let script = format!("ping -n 3 127.0.0.1 >nul & {}", removals.join(" & "));
    // raw_arg: `args` would escape the inner quotes as `\"`, which cmd does not understand, so every rmdir would fail silently.
    let _ = Command::new("cmd").raw_arg(format!("/C \"{script}\"")).creation_flags(CREATE_NO_WINDOW).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_with_a_space_in_its_name_is_really_removed() {
        let dir = std::env::temp_dir().join("cue cleanup test");
        std::fs::create_dir_all(dir.join("inner")).unwrap();
        remove_dirs_after_exit(std::slice::from_ref(&dir));
        let gone = (0..40).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(250));
            !dir.exists()
        });
        assert!(gone, "the folder is still there");
    }
}
