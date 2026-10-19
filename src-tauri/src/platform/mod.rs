//! Purpose: OS-specific code behind one API, so the rest of Cue never mentions Win32 or AppKit.
//! Contents: Point / MenuEntry / show_menu — the tray popup menu; InsertOptions — how text is delivered; insert / insert_rich / replace — put text, or text with its HTML, in the focused app, or replace what was just typed; mute_output / unmute_output — silence the speakers while recording; text_before_cursor / press_submit — fit a dictation to its surroundings, press Enter after it; copy_selection — read the selected text (and its HTML); grab_region / recognize_text / ocr_languages — let the user drag over the screen, read the text in the picture, and list the languages Windows can read; foreground_window / focus_window / foreground_app / list_apps / copy_text — which window and program has focus (its executable), the programs a person could name (each with its executable), bring a window back, and put text on the clipboard; overlay — recording pill in the user's theme (init, show, hide, dismiss, caption, notice); beep — the system beep;
//! open_url — open a web page in the default browser; start_program — run a program (an installer) the way Explorer would; work_area_of — the work area and scale of the monitor a window or the pointer is on; trim_working_set — give freed memory back to the OS; performance_cores / disable_power_throttling / hide_console / end_with_parent / harden_process — process helpers (speech worker, text model server); port_owner — which process listens on a local port; remove_dirs_after_exit — delete locked directories after the app exits; selection_anchor — where the selected text is and the work area of its monitor; extract_zip — unpack a zip file. `win` is the real backend; `stub` keeps other targets compiling
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

/// One row of a popup menu. `icon` names an icon from the menu icon set (app, settings, power, bug, warn).
pub enum MenuEntry {
    Item { id: u32, label: String, icon: Option<&'static str> },
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

/// What the typing watcher reports: a character typed (Space, Tab and Enter included), Backspace, or a break (arrows, shortcuts, another window, a pause) after which what was typed so far no longer counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typed {
    Char(char),
    Backspace,
    Reset,
}

/// A key `press` sends.
#[derive(Clone, Copy)]
pub enum Key {
    Backspace,
    Left,
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

pub mod flow;
pub mod place;

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
