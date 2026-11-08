//! File: stub.rs
//! Purpose: Placeholder backend for non-Windows targets (macOS comes later); every call is a harmless no-op or error.
//! Contents: the same surface as `win`, each call a harmless no-op or a user-readable error: insert / replace / replace_rich (the shared `InsertError`), clipboard, selection, overlay, card, popup menu, keyboard thread, process helpers, machine facts and the like. `unavailable` — the one insertion failure;

use super::{InsertError, InsertOptions, MenuEntry, Point, Theme};

/// The failure every insertion call gives here: nothing ever went in and nothing was copied.
fn unavailable() -> InsertError {
    InsertError::Failed { why: "Text insertion is not available on this platform yet.".into(), copied: false }
}

/// Twin of the Windows `insert`: always fails, with the same `InsertError`.
pub fn insert(_text: &str, _options: &InsertOptions) -> Result<(), InsertError> {
    Err(unavailable())
}

/// Twin of the Windows `replace`: always fails, with the same `InsertError`.
pub fn replace(_erase: usize, _text: &str, _back: usize, _options: &InsertOptions) -> Result<(), InsertError> {
    Err(unavailable())
}

/// Twin of the Windows `replace_rich`: always fails, with the same `InsertError`.
pub fn replace_rich(_erase: usize, _text: &str, _html: &str, _back: usize, _options: &InsertOptions) -> Result<(), InsertError> {
    Err(unavailable())
}

/// Twin of the Windows `work_area_of`: no monitor is known here.
pub fn work_area_of(_handle: isize) -> Option<(super::Rect, f32)> {
    None
}

/// Twin of the Windows `run_installer`: always fails, nothing is started.
pub fn run_installer(_path: &std::path::Path, _args: &str) -> Result<bool, String> {
    Err("Starting the installer is not available on this platform yet.".into())
}

/// Twin of the Windows `open_url`: always fails, nothing is opened.
pub fn open_url(_url: &str) -> Result<(), String> {
    Err("Opening a web page is not available on this platform yet.".into())
}

/// Twin of the Windows `grab_region`: always fails, the screen is not read.
pub fn grab_region(_theme: &Theme) -> Result<Option<super::Image>, String> {
    Err("Reading text from the screen is not available on this platform yet.".into())
}

/// Twin of the Windows `recognize_image_file`: always fails.
pub fn recognize_image_file(_path: &std::path::Path) -> Result<String, String> {
    Err("Text recognition is only available on Windows.".into())
}

/// Twin of the Windows `recognize_text`: always fails.
pub fn recognize_text(_image: &super::Image) -> Result<String, String> {
    Err("Reading text from the screen is not available on this platform yet.".into())
}

/// Twin of the Windows `ocr_languages`: no language is readable here.
pub fn ocr_languages() -> Vec<String> {
    Vec::new()
}

/// Twin of the Windows `foreground_window`: 0, no window is known.
pub fn foreground_window() -> isize {
    0
}

/// Twin of the Windows `focus_window`: false, no window is brought back.
pub fn focus_window(_window: isize) -> bool {
    false
}

/// Twin of the Windows `mute_output`: nothing is muted, so there is nothing to restore.
pub fn mute_output() -> Result<Option<String>, String> {
    Ok(None)
}

/// Twin of the Windows `unmute_output`: nothing was muted.
pub fn unmute_output(_id: &str) -> Result<(), String> {
    Ok(())
}

/// Twin of the Windows `text_before_cursor`: no text is known.
pub fn text_before_cursor() -> Option<String> {
    None
}

/// Twin of the Windows `list_apps`: no programs are listed.
pub fn list_apps() -> Vec<String> {
    Vec::new()
}

/// Twin of the Windows `foreground_app`: empty, no program is known.
pub fn foreground_app() -> String {
    String::new()
}

/// Twin of the Windows `copy_selection`: always fails.
pub fn copy_selection() -> Result<Option<super::Copied>, String> {
    Err("Reading the selection is not available on this platform yet.".into())
}

/// Twin of the Windows `copy_text`: always fails, there is no clipboard.
pub fn copy_text(_text: &str) -> Result<(), String> {
    Err("The clipboard is not available on this platform yet.".into())
}

/// Twin of the Windows `trim_working_set`: nothing to give back.
pub fn trim_working_set() {}

/// Twin of the Windows `total_ram_mb`: 0, unknown.
pub fn total_ram_mb() -> u64 {
    0
}

/// Twin of the Windows `weak_machine`: false.
pub fn weak_machine() -> bool {
    false
}

/// Twin of the Windows `machine_summary`: empty.
pub fn machine_summary() -> String {
    String::new()
}

/// Twin of the Windows `selection_anchor`: no selection is located.
pub fn selection_anchor(_selected: Option<super::Rect>) -> Option<(super::Rect, super::Rect)> {
    None
}

/// Twin of the Windows `extract_zip`: always fails.
pub fn extract_zip(_archive: &std::path::Path, _target: &std::path::Path) -> Result<(), String> {
    Err("Unpacking is not available on this platform yet.".into())
}

/// Twin of the Windows `free_bytes`: always fails.
pub fn free_bytes(_dir: &std::path::Path) -> Result<u64, String> {
    Err("Disk space is not available on this platform yet.".into())
}

/// Twin of the Windows `performance_cores`: the logical processor count.
pub fn performance_cores() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Twin of the Windows `disable_power_throttling`: nothing to turn off.
pub fn disable_power_throttling() {}

/// Twin of the Windows `background_thread`: the priority is left alone.
pub fn background_thread() {}

/// Twin of the Windows `background_engine`: the command is left alone.
pub fn background_engine(_command: &mut std::process::Command) {}

/// Twin of the Windows `hide_console`: the command is left alone.
pub fn hide_console(_command: &mut std::process::Command) {}

/// Twin of the Windows `harden_process`: nothing to restrict.
pub fn harden_process() {}

/// Twin of the Windows `end_with_parent`: true, nothing to tie.
pub fn end_with_parent(_child: &std::process::Child) -> bool {
    true
}

/// Twin of the Windows `port_owner`: no owner is known.
pub fn port_owner(_port: u16) -> Option<u32> {
    None
}

/// Twin of the Windows `remove_dirs_after_exit`: nothing is removed.
pub fn remove_dirs_after_exit(_dirs: &[std::path::PathBuf]) -> Result<(), String> {
    Ok(())
}

/// Twin of the Windows `show_menu`: no menu opens, as if dismissed.
pub fn show_menu(_entries: Vec<MenuEntry>, _at: Point, _theme: &Theme, _lease: super::PopupOpen) -> Result<Option<u32>, String> {
    Ok(None)
}

/// Twin of the Windows `watch_typing`: always fails.
pub fn watch_typing(_handler: Box<dyn Fn(super::Typed) + Send + Sync>) -> Result<(), String> {
    Err("Snippet expansion is not available on this platform yet.".into())
}

/// Twin of the Windows `stop_typing`: nothing runs.
pub fn stop_typing() {}

/// The twin of the Windows `PumpHandle`: no keyboard thread exists here.
pub struct PumpHandle;

impl PumpHandle {
    /// Nothing runs.
    pub fn stop(self) {}
}

/// The twin of the Windows `start_input_pump`: global shortcuts do not exist here, so it returns an error and never a thread.
pub fn start_input_pump(
    _thread: std::sync::Arc<std::sync::atomic::AtomicU32>,
    _runtime: crate::input::Runtime<Box<dyn crate::input::Sink>>,
) -> Result<PumpHandle, String> {
    Err("Global shortcuts are not available on this platform yet.".into())
}

/// The twin of the Windows `wake_input`: no thread to wake.
pub fn wake_input(_thread: u32) {}

/// The twin of the Windows `send_keys`: nothing is sent.
pub fn send_keys(_batch: &crate::input::KeyBatch) -> usize {
    0
}

/// The twin of the Windows `send_menu_mask`: nothing is sent.
pub fn send_menu_mask() {}

/// The twin of the Windows `os_modifiers`: none are held.
pub fn os_modifiers() -> crate::input::Mods {
    crate::input::Mods::NONE
}

/// The twin of the Windows `lift_blocked`: nothing can be lifted here.
pub fn lift_blocked() -> bool {
    true
}

/// Twin of the Windows `watch_clipboard`: always fails.
pub fn watch_clipboard(_handler: Box<dyn Fn(String) + Send + Sync>) -> Result<(), String> {
    Err("Not supported on this platform.".into())
}

/// Twin of the Windows `stop_clipboard_watch`: nothing runs.
pub fn stop_clipboard_watch() {}

/// Twin of the Windows `insert_slowly`: always fails.
pub fn insert_slowly(_erase: usize, _text: &str, _back: usize) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
}

/// Twin of the Windows `uia_selection`: no selection is known.
pub fn uia_selection(_limit: usize) -> Option<super::UiaSelection> {
    None
}

/// Twin of the Windows `focused_is_password`: false.
pub fn focused_is_password() -> bool {
    false
}

/// Twin of the Windows `input_blocked`: false.
pub fn input_blocked() -> bool {
    false
}

/// Twin of the Windows `clipboard_text`: no text.
pub fn clipboard_text() -> Option<String> {
    None
}

/// Twin of the Windows `beep`: silent.
pub fn beep(_volume: u8) {}

/// Twin of the Windows `system_uses_dark_theme`: false.
pub fn system_uses_dark_theme() -> bool {
    false
}

/// Twin of the Windows `float_window`: the window is left alone.
pub fn float_window(_window: isize) {}

/// Twin of the Windows `local_time`: 1970-01-01 until this platform has a clock, so placeholders still work, with that date.
pub fn local_time() -> super::LocalTime {
    super::LocalTime { year: 1970, month: 1, day: 1, weekday: 4, ..Default::default() }
}

/// Twin of the Windows `card` module: the dictionary card.
pub mod card {
    /// Twin of the Windows `card::show`: nothing is shown, so the card counts as closed at once.
    pub fn show(request: crate::platform::CardRequest) {
        (request.on_closed)();
    }
    /// Twin of the Windows `card::hide`: false, no card was open.
    pub fn hide() -> bool {
        false
    }
}

/// Twin of the Windows `overlay` module: the recording pill.
pub mod overlay {
    use crate::platform::Theme;

    /// Twin of `overlay::init`: nothing to start.
    pub fn init() -> std::io::Result<()> {
        Ok(())
    }
    /// Twin of `overlay::show`: nothing is shown.
    pub fn show(_id: u64, _top: bool, _theme: Theme) {}
    /// Twin of `overlay::hide`: nothing is shown.
    pub fn hide(_id: u64) {}
    /// Twin of `overlay::dismiss`: nothing is shown.
    pub fn dismiss() {}
    /// Twin of `overlay::caption`: no pill to caption.
    pub fn caption(_index: usize, _text: &str) {}
    /// Twin of `overlay::notice`: no pill to show it on.
    pub fn notice(_text: &str, _top: bool, _theme: Theme) {}
}
