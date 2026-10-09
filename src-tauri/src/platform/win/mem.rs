//! Purpose: Hand freed pages back to Windows.
//! Contents: trim_working_set — empties this process's working set.

use windows::Win32::System::{ProcessStatus::K32EmptyWorkingSet, Threading::GetCurrentProcess};

pub fn trim_working_set() {
    let _ = unsafe { K32EmptyWorkingSet(GetCurrentProcess()) };
}
