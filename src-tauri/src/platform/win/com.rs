//! File: com.rs
//! Purpose: Keep COM initialised on the current thread for as long as a guard lives (audio devices, UI Automation).
//! Contents: Com — the guard; `Com::init()` on a short-lived thread, dropped when the work is done.

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// Holds whether this guard owns an initialisation: S_OK and S_FALSE must be balanced by an uninitialise, a failure (the thread already
/// has an apartment of another kind, which is still usable) must not be.
pub struct Com(bool);

impl Com {
    /// Initialises COM (multithreaded) on this thread. Safety: `CoInitializeEx` has no preconditions; the guard balances a success with `CoUninitialize` on the same thread when dropped.
    pub fn init() -> Com {
        Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Com {
    /// Uninitialises COM when this guard initialised it. Safety: pairs the successful `CoInitializeEx` of `init` on the same thread.
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}
