//! File: lab/installed.rs
//! Purpose: `cue --lab installed <models folder>`: the speech models installed in a folder, by production's own rule (`catalog::installed_in`), for the bench and the lab.
//! Contents: list — one installed model id per line.

use std::path::Path;

use crate::speech::catalog;

/// Lists the speech models whose files are all present at their catalog sizes (with the shared VAD for non-streaming models). Input: the models folder. Returns one id per line, or an error when the folder does not exist.
pub fn list(root: &Path) -> Result<String, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    Ok(catalog::all().filter(|m| catalog::installed_in(root, m)).map(|m| format!("{}\n", m.id)).collect())
}
