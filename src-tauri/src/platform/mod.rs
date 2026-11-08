//! File: mod.rs
//! Purpose: OS-specific code behind one API, so the rest of Cue never mentions Win32 or AppKit.
//! Contents: Point / MenuEntry / show_menu — the tray popup menu (Ok(None) is a dismissed menu, Err a menu that could not open); set_reporter / report — where a native window sends a failure the user should see; InsertOptions — how text is delivered; insert / replace_rich / replace — put text, or text with its HTML, in the focused app, or replace what was just typed; mute_output / unmute_output — silence the speakers while recording; text_before_cursor — fit a dictation to its surroundings; Submit / InsertError — the key pressed after an insertion and how a delivery ended (see `delivery`); copy_selection — read the selected text (and its HTML); grab_region / recognize_text / recognize_image_file / ocr_languages — let the user drag over the screen, read the text in the picture (or in a picture file), and list the languages Windows can read; foreground_window / focus_window / foreground_app / list_apps / copy_text — which window and program has focus (its executable), the programs a person could name (each with its executable), bring a window back, and put text on the clipboard; overlay — recording pill in the user's theme (init, show, hide, dismiss, caption, notice); beep — a short alert tone at a given volume; float_window — a window above the others; watch_clipboard / stop_clipboard_watch — text the user copies, while a note collects it; start_input_pump / wake_input / send_keys / send_menu_mask / os_modifiers / lift_blocked — the keyboard thread of the `input` module and its marked key sending;
//! open_url — open a web page in the default browser; run_installer — start the verified per-user installer silently (`CreateProcessW`, no shell prompt); work_area_of — the work area and scale of the monitor a window or the pointer is on; trim_working_set — give freed memory back to the OS; total_ram_mb / weak_machine / machine_summary — installed memory, whether the PC is small (lighter models, longer waits) and a one-line description for the log; performance_cores / disable_power_throttling / background_thread / hide_console / background_engine / end_with_parent / harden_process — process helpers (speech worker, text model server); port_owner — which process listens on a local port; remove_dirs_after_exit — delete locked directories after the app exits; selection_anchor — where the selected text is and the work area of its monitor; extract_zip — unpack a zip file; free_bytes — room left for the user on the drive holding a folder. `win` is the real backend; `stub` keeps other targets compiling
//! until the macOS twins exist.

/// A point on the screen in physical pixels.
#[derive(Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// A rectangle on the screen in physical pixels.
#[derive(Clone, Copy)]
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

mod delivery;
pub mod flow;
pub mod place;

pub use delivery::{InsertError, InsertOptions, Submit};

/// The function a native surface calls to tell the user about a failure.
type Reporter = Box<dyn Fn(&str) + Send + Sync>;

static REPORTER: std::sync::OnceLock<Reporter> = std::sync::OnceLock::new();

/// Registers where a native surface (pill, card) sends a failure the user should see; the app wires it to `status::report` once at start. Input: the reporter.
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

/// What UI Automation says is selected: the text (when short enough) and where its first line is on the screen.
pub struct UiaSelection {
    pub text: String,
    pub rect: Option<Rect>,
}

/// A floating card that explains the selected text (the dictionary popup): sections of numbered meanings, drawn natively beside the text.
pub struct CardSection {
    /// The word shown large at the head of its first section; later sections of the same word have none.
    pub heading: Option<String>,
    /// Part of speech.
    pub label: String,
    /// Meanings, each with an optional example that follows it in italics.
    pub items: Vec<(String, Option<String>)>,
}

/// What the dictionary card shows and where it goes.
pub struct CardRequest {
    /// The large title over the first section: "Dictionary", or "German - English" for a pack in another language.
    pub title: String,
    pub sections: Vec<CardSection>,
    /// The selected text on the screen and the work area of its monitor, in physical pixels.
    pub target: Rect,
    pub work: Rect,
    pub theme: Theme,
    /// The window the word is in (`foreground_window`): the card goes away when it is no longer the one in front.
    pub source: isize,
    /// Called on the card's thread when it closes for any reason.
    pub on_closed: fn(),
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
