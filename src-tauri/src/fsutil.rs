//! File: fsutil.rs
//! Purpose: The one way Cue writes a file (synced temporary file beside the target, then a rename over it) and the one way it keeps a damaged file aside.
//! Contents: write_atomic — replace a file with bytes so a crash or a full disk leaves the old file or the new one, never a half-written one; set_aside — move a damaged file to the first free recovery name.

use std::io::Write;
use std::path::{Path, PathBuf};

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

/// The first free name for a recovery copy of `<stem>.json`: `<stem>.damaged.json`, then `<stem>.damaged-2.json` and so on, so an earlier copy is never replaced. Input: the damaged file's path. Returns the free path.
fn free_copy_name(path: &Path) -> PathBuf {
    let stem = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    std::iter::once(path.with_file_name(format!("{stem}.damaged.json")))
        .chain((2..).map(|n| path.with_file_name(format!("{stem}.damaged-{n}.json"))))
        .find(|p| !p.exists())
        .expect("endless range")
}

/// Moves a damaged file to the first free recovery name (`free_copy_name`), so the next save cannot destroy what was in it and no earlier copy is lost. Input: the file's path. Returns the copy's file name, or the I/O error when it could not be moved (the file then stays where it is).
pub fn set_aside(path: &Path) -> std::io::Result<String> {
    let copy = free_copy_name(path);
    std::fs::rename(path, &copy)?;
    Ok(copy.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests;
