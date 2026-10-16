//! Purpose: Unpack a zip file with the `tar` that ships with Windows 10 and later (it reads zip), so no archive library is needed.
//! Contents: extract_zip — unpack an archive into a folder.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn extract_zip(archive: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|e| format!("Could not create the folder: {e}"))?;
    // Windows' own tar by its full path, so a program called `tar` earlier on the PATH cannot stand in for it.
    let system = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let mut tar = Command::new(system.join("System32").join("tar.exe"));
    tar.arg("-xf").arg(archive).arg("-C").arg(target);
    super::hide_console(&mut tar);
    match tar.status() {
        Ok(done) if done.success() => Ok(()),
        Ok(_) => Err("The downloaded file could not be unpacked.".into()),
        Err(e) => Err(format!("Could not unpack the downloaded file: {e}")),
    }
}
