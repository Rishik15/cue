//! File: fsutil.rs
//! Purpose: The one way Cue writes a file: synced temporary file beside the target, then a rename over it.
//! Contents: write_atomic — replace a file with bytes so a crash or a full disk leaves the old file or the new one, never a half-written one.

use std::io::Write;
use std::path::Path;

/// Replaces `path` with `bytes`: the folder is created when missing, the bytes go to `<file name>.tmp` beside it, are synced, and the rename replaces the old file in one step. Inputs: the target path and the bytes. Returns the I/O result; on any error the temporary file is removed and the old file is untouched.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let name = path.file_name().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name"))?;
    let temp = path.with_file_name(format!("{}.tmp", name.to_string_lossy()));
    let result = write_synced(&temp, bytes).and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// Creates or truncates a file, writes the bytes and syncs it to disk. Inputs: path and bytes. Returns the I/O result.
fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests;
