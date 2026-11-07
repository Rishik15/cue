//! Purpose: Ask Windows how much room a download may use on a drive.
//! Contents: free_bytes — the bytes the current user can still write to the drive that holds a folder (quotas included).

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

/// Bytes available to this user on the drive holding `dir`, which must exist. Err carries the OS message.
pub fn free_bytes(dir: &Path) -> Result<u64, String> {
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut free = 0u64;
    unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut free), None, None) }.map_err(|e| e.to_string())?;
    Ok(free)
}
