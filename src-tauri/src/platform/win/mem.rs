//! File: mem.rs
//! Purpose: Hand freed pages back to Windows.
//! Contents: trim_working_set — empties this process's working set.

use windows::Win32::System::{ProcessStatus::K32EmptyWorkingSet, Threading::GetCurrentProcess};

/// Asks Windows to give back the pages of this process that are not in use. Safety: empties the working set of the current process handle, which is always valid.
pub fn trim_working_set() {
    let _ = unsafe { K32EmptyWorkingSet(GetCurrentProcess()) };
}
