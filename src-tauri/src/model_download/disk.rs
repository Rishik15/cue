//! File: model_download/disk.rs
//! Purpose: The seam between a download and the drive: free room and folder creation. The production adapter is `RealDisk`; tests plug in one with a scripted amount of room.
//! Contents: Disk — the trait; RealDisk — the production adapter.

use std::io;
use std::path::Path;

/// What a download asks of the drive besides reading and writing its own files.
pub trait Disk: Send + Sync {
    /// Bytes free on the drive that holds `dir`.
    /// shortcut: an implementation answers unlimited when the drive cannot say, so a failed query never blocks a download; upgrade: refuse with a readable message once `platform::free_bytes` has a failure worth showing.
    fn free_room(&self, dir: &Path) -> u64;

    /// Creates `dir` and its parents.
    fn create_dir_all(&self, dir: &Path) -> io::Result<()>;
}

/// The real drive.
pub struct RealDisk;

impl Disk for RealDisk {
    /// Room on the drive of `dir` (or its nearest existing parent); unlimited when Windows cannot say.
    fn free_room(&self, dir: &Path) -> u64 {
        dir.ancestors().find(|d| d.exists()).and_then(|d| crate::platform::free_bytes(d).ok()).unwrap_or(u64::MAX)
    }

    /// Creates the folder on disk.
    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)
    }
}
