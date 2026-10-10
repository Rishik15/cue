//! Purpose: OS-specific code behind one API, so the rest of Cue never mentions Win32 or AppKit.
//! Contents: Point / MenuEntry / show_menu — the tray popup menu; InsertOptions — how text is delivered; insert — put text in the focused app; foreground_window / copy_text — which window has focus, and put text on the clipboard; overlay — recording pill in the user's theme (init, show, hide, dismiss, caption, notice);
//! trim_working_set — give freed memory back to the OS; performance_cores / disable_power_throttling / hide_console — speech worker process helpers; remove_dirs_after_exit — delete locked directories after the app exits. `win` is the real backend; `stub` keeps other targets compiling
//! until the macOS twins exist.

/// A point on the screen in physical pixels.
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// The user's Cue theme as the native overlay needs it: the saved role colours (JSON, may be empty) and the Appearance mode
/// ("System", "Light" or "Dark"). The overlay resolves the dark or light variant itself, so a Windows light/dark switch is followed.
#[derive(Clone, Default)]
pub struct Theme {
    pub colors: String,
    pub mode: String,
}

/// One row of a popup menu. `icon` names an icon from the menu icon set (app, settings, power, bug, warn).
pub enum MenuEntry {
    Item { id: u32, label: String, icon: Option<&'static str>, enabled: bool },
    Separator,
}

/// How `insert` delivers text; the defaults are what most people want.
pub struct InsertOptions {
    /// Skip typing and always paste, even for short single-line text.
    pub always_paste: bool,
    /// Put the previous clipboard back after a paste.
    pub restore_clipboard: bool,
}

impl Default for InsertOptions {
    fn default() -> Self {
        InsertOptions { always_paste: false, restore_clipboard: true }
    }
}

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::*;
