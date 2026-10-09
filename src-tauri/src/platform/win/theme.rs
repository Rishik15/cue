//! Purpose: Read whether Windows is set to dark mode for apps, which the tray menu follows.
//! Contents: system_uses_dark_theme — registry `AppsUseLightTheme`; dark when it is 0 (light when unreadable is the OS default).

use windows::core::w;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

pub fn system_uses_dark_theme() -> bool {
    let (mut light, mut size) = (1u32, size_of::<u32>() as u32);
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut light as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    status.0 == 0 && light == 0
}
