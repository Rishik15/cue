//! File: clock.rs
//! Purpose: The local date and time for snippet placeholders (Windows).
//! Contents: local_time — now, in the user's time zone.

use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::platform::LocalTime;

/// The local date and time from Windows. Safety: `GetLocalTime` has no preconditions. Safety: Returns the current local time by value.
pub fn local_time() -> LocalTime {
    let t = unsafe { GetLocalTime() };
    LocalTime {
        year: i32::from(t.wYear),
        month: u32::from(t.wMonth),
        day: u32::from(t.wDay),
        hour: u32::from(t.wHour),
        minute: u32::from(t.wMinute),
        second: u32::from(t.wSecond),
        weekday: u32::from(t.wDayOfWeek),
    }
}
