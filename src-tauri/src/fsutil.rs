//! File: fsutil.rs
//! Purpose: The one way Cue writes a file (synced temporary file beside the target, then a rename over it) and the one way it keeps a damaged file aside.
//! Contents: write_atomic — replace a file with bytes so a crash or a full disk leaves the old file or the new one, never a half-written one; commit — sync a finished file written elsewhere (a verified download) and rename it into place the same way; set_aside — move a damaged file to the first free recovery name.

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

/// Moves a finished file into place the way `write_atomic` does: syncs `part` to disk, then renames it over `target`. For files streamed to disk outside `write_atomic` (a verified download), so a crash after the rename cannot leave a target with unsynced bytes. Inputs: the finished file and the target path. Returns the I/O result; on error `part` stays where it is and the target is untouched.
pub fn commit(part: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new().write(true).open(part)?.sync_all()?;
    std::fs::rename(part, target)
}

/// Creates or truncates a file, writes the bytes and syncs it to disk. Inputs: path and bytes. Returns the I/O result.
fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// How many recovery names `free_copy_name` tries before it gives up.
const MAX_COPIES: usize = 1000;

/// The first free name for a recovery copy of `<stem>.json`: `<stem>.damaged.json`, then `<stem>.damaged-2.json` and so on up to `MAX_COPIES` names, so an earlier copy is never replaced. Inputs: the damaged file's path and the existence check (`Path::try_exists`). Returns the free path, or an I/O error when a name's existence cannot be checked or every name is taken.
fn free_copy_name(path: &Path, exists: impl Fn(&Path) -> std::io::Result<bool>) -> std::io::Result<PathBuf> {
    let stem = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let names = std::iter::once(format!("{stem}.damaged.json")).chain((2..=MAX_COPIES).map(|n| format!("{stem}.damaged-{n}.json")));
    for name in names {
        let copy = path.with_file_name(name);
        if !exists(&copy)? {
            return Ok(copy);
        }
    }
    Err(std::io::Error::other(format!("all {MAX_COPIES} recovery names for {stem}.json are taken")))
}

/// Moves a damaged file to the first free recovery name (`free_copy_name`), so the next save cannot destroy what was in it and no earlier copy is lost. Input: the file's path. Returns the copy's file name, or the I/O error when no free name was found or it could not be moved (the file then stays where it is).
pub fn set_aside(path: &Path) -> std::io::Result<String> {
    let copy = free_copy_name(path, Path::try_exists)?;
    std::fs::rename(path, &copy)?;
    Ok(copy.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests;
