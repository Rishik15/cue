//! File: mod.rs
//! Purpose: OS-specific code behind one API, so the rest of Cue never mentions Win32 or AppKit.
//! Contents: Point / MenuEntry / show_menu — the tray popup menu (Ok(None) is a dismissed menu, Err a menu that could not open); set_reporter / report — where a native window sends a failure the user should see; InsertOptions — how text is delivered; insert / replace_rich / replace — put text, or text with its HTML, in the focused app, or replace what was just typed; mute_output / unmute_output — silence the speakers while recording; text_before_cursor — fit a dictation to its surroundings; Submit / InsertError — the key pressed after an insertion and how a delivery ended (see `delivery`); capture — read the selected text (and its HTML) as a `Capture`, for the dictionary and the text transform alike; capture_target — the token of the window and field text is for; Receipt / Target / Capture — see `delivery`; grab_region / recognize_text / recognize_image_file / ocr_languages — let the user drag over the screen, read the text in the picture (or in a picture file), and list the languages Windows can read; foreground_window / focus_window / foreground_app / list_apps / copy_text — which window and program has focus (its executable), the programs a person could name (each with its executable), bring a window back, and put text on the clipboard; overlay — recording pill in the user's theme (init, show, hide, dismiss, caption, notice); uia_selection_for / displays / caret_in / pointer_box / in_front / watch_front — the dictionary window's selection read (bound to the shortcut's target, one rectangle per line), every monitor's work area and scale, the caret of a window, a box round the pointer, whether the word's window is still in front, and the hooks that close the window when the user clicks away; play_wav — play a finished WAV file without waiting (used by `sound`); float_window / place_window — a window above the others, and moved and sized in one step; watch_clipboard / stop_clipboard_watch — text the user copies, while a note collects it; start_input_pump / wake_input / send_keys / send_menu_mask / os_modifiers / lift_blocked — the keyboard thread of the `input` module and its marked key sending;
//! open_url — open a web page in the default browser; run_installer — start the verified per-user installer silently (`CreateProcessW`, no shell prompt); work_area_of — the work area and scale of the monitor a window or the pointer is on; trim_working_set — give freed memory back to the OS; total_ram_mb / weak_machine / machine_summary — installed memory, whether the PC is small (lighter models, longer waits) and a one-line description for the log; performance_cores / disable_power_throttling / background_thread / hide_console / process_killer / background_engine / end_with_parent / harden_process — process helpers (speech worker, text model server); port_owner — which process listens on a local port; remove_dirs_after_exit / run_cleanup_job / wait_for_pending_cleanup — delete locked directories once the app's process has exited (the app runs itself as the helper, see `cleanup_job`), run that helper, and make a new start wait for it; selection_anchor — where the selected text is and the work area and scale of its monitor; extract_zip — unpack a zip file; free_bytes — room left for the user on the drive holding a folder. `win` is the real backend; `stub` keeps other targets compiling
//! until the macOS twins exist.

/// A point on the screen in physical pixels.
#[derive(Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// A rectangle on the screen in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// What copying the selection gave: its text, and the HTML the program also put on the clipboard (a web page, a document) when there was any.
pub struct Copied {
    pub text: String,
    pub html: Option<String>,
    /// False when the clipboard the user had before could not be put back: the caller tells them.
    pub restored: bool,
    /// The place the text was copied from, as it was when Ctrl+C went out: delivery checks this token, so the result goes back only to the field it was read from.
    pub target: Target,
}

/// A picture of part of the screen: `w` by `h` pixels of BGRA, top row first.
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub bgra: Vec<u8>,
}

/// The user's theme as the native windows need it: the family id (see `src/themes.json`) and the Appearance mode
/// ("System", "Light" or "Dark"). Each window resolves the colours itself, so a Windows light/dark switch is followed.
#[derive(Clone, Default)]
pub struct Theme {
    pub family: String,
    pub mode: String,
}

/// How the tray menu asks the input module for its lease: the menu's physical bounds and the inbox for what the input module tells it; Err is a sentence for the user.
pub type PopupOpen<'a> =
    &'a dyn Fn(crate::input::Rect, Box<dyn Fn(crate::input::PopupKey) -> bool + Send>) -> Result<crate::input::PopupLease, String>;

/// One row of a popup menu. `icon` names an icon from the menu icon set (app, settings, power, bug, warn).
pub enum MenuEntry {
    Item { id: u32, label: String, icon: Option<&'static str> },
    Separator,
}

/// What the typing watcher reports: a character typed (Space, Tab and Enter included), Backspace, or a break (arrows, shortcuts, another window, a pause) after which what was typed so far no longer counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typed {
    Char(char),
    Backspace,
    Reset,
}

/// The local date and time; `weekday` counts from Sunday = 0.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub weekday: u32,
}

#[cfg(any(windows, test))]
pub(crate) mod caption_queue;
pub(crate) mod cleanup_job;
mod delivery;
mod os_values;
pub mod place;
mod reading;
#[cfg(any(windows, test))]
pub(crate) mod typing_keys;
#[cfg(any(windows, test))]
pub(crate) mod typing_policy;
#[cfg(any(windows, test))]
pub(crate) mod watcher_state;

pub use delivery::{Capture, Caret, ClipboardEnd, Failure, InsertError, InsertOptions, Placement, Receipt, Submit, Target};
pub use os_values::{env_var, random_u64};
pub use reading::{Reading, Stop};

/// The function a native surface calls to tell the user about a failure.
type Reporter = Box<dyn Fn(&str) + Send + Sync>;

static REPORTER: std::sync::OnceLock<Reporter> = std::sync::OnceLock::new();

/// Registers where a native surface (pill, notices) sends a failure the user should see; the app wires it to `status::report` once at start. Input: the reporter.
pub fn set_reporter(reporter: impl Fn(&str) + Send + Sync + 'static) {
    let _ = REPORTER.set(Box::new(reporter));
}

/// Tells the user about a failure of a native surface through the registered reporter; the log has it already when none is set. Input: the sentence to show.
#[cfg(windows)]
pub(crate) fn report(text: &str) {
    if let Some(reporter) = REPORTER.get() {
        reporter(text);
    }
}

/// What UI Automation says is selected: the text (when short enough), the rectangle that holds every visible line of it, and each visible line's own rectangle (a card is placed by the lines, never by their union).
pub struct UiaSelection {
    pub text: String,
    pub rect: Option<Rect>,
    pub lines: Vec<Rect>,
}

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::*;

#[cfg(test)]
mod tests;
