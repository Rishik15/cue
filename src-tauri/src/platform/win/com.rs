//! Purpose: Keep COM initialised on the current thread for as long as a guard lives (audio devices, UI Automation).
//! Contents: Com — the guard; `Com::init()` on a short-lived thread, dropped when the work is done.

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

pub struct Com;

impl Com {
    pub fn init() -> Com {
        // An error means the thread already has a COM apartment, which is fine to use; the balancing uninitialise below is still correct.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Com
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
