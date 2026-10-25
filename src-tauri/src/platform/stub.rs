//! Purpose: Placeholder backend for non-Windows targets (macOS comes later); every call is a harmless no-op or error.
//! Contents: same surface as `win`: insert, overlay, trim_working_set.

use super::{InsertOptions, MenuEntry, Point, Theme};

/// The twin of the Windows `InsertError`: text insertion does not exist here, so nothing ever went in.
#[derive(Debug)]
pub struct InsertError(String);

impl InsertError {
    pub fn text_went_in(&self) -> bool {
        false
    }

    pub fn pill(&self) -> &'static str {
        "Could not insert the text"
    }
}

impl std::fmt::Display for InsertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn insert(_text: &str, _options: &InsertOptions) -> Result<(), InsertError> {
    Err(InsertError("Text insertion is not available on this platform yet.".into()))
}

pub fn replace(_erase: usize, _text: &str, _back: usize, _options: &InsertOptions) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
}

pub fn insert_rich(_text: &str, _html: &str, _options: &InsertOptions) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
}

pub fn work_area_of(_handle: isize) -> Option<(super::Rect, f32)> {
    None
}

pub fn run_installer(_path: &std::path::Path, _args: &str) -> Result<bool, String> {
    Err("Starting the installer is not available on this platform yet.".into())
}

pub fn open_url(_url: &str) -> Result<(), String> {
    Err("Opening a web page is not available on this platform yet.".into())
}

pub fn grab_region(_theme: &Theme) -> Result<Option<super::Image>, String> {
    Err("Reading text from the screen is not available on this platform yet.".into())
}

pub fn recognize_image_file(_path: &std::path::Path) -> Result<String, String> {
    Err("Text recognition is only available on Windows.".into())
}

pub fn recognize_text(_image: &super::Image) -> Result<String, String> {
    Err("Reading text from the screen is not available on this platform yet.".into())
}

pub fn ocr_languages() -> Vec<String> {
    Vec::new()
}

pub fn foreground_window() -> isize {
    0
}

pub fn focus_window(_window: isize) -> bool {
    false
}

pub fn mute_output() -> Result<Option<String>, String> {
    Ok(None)
}

pub fn unmute_output(_id: &str) -> Result<(), String> {
    Ok(())
}

pub fn text_before_cursor() -> Option<String> {
    None
}

pub fn press_submit(_with_ctrl: bool) -> Result<(), String> {
    Ok(())
}

pub fn list_apps() -> Vec<String> {
    Vec::new()
}

pub fn foreground_app() -> String {
    String::new()
}

pub fn copy_selection() -> Result<Option<super::Copied>, String> {
    Err("Reading the selection is not available on this platform yet.".into())
}

pub fn copy_text(_text: &str) -> Result<(), String> {
    Err("The clipboard is not available on this platform yet.".into())
}

pub fn trim_working_set() {}

pub fn total_ram_mb() -> u64 {
    0
}

pub fn weak_machine() -> bool {
    false
}

pub fn machine_summary() -> String {
    String::new()
}

pub fn selection_anchor(_selected: Option<super::Rect>) -> Option<(super::Rect, super::Rect)> {
    None
}

pub fn extract_zip(_archive: &std::path::Path, _target: &std::path::Path) -> Result<(), String> {
    Err("Unpacking is not available on this platform yet.".into())
}

pub fn performance_cores() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

pub fn disable_power_throttling() {}
pub fn background_thread() {}

pub fn rearm_keyboard_hook() {}

pub fn background_engine(_command: &mut std::process::Command) {}

pub fn hide_console(_command: &mut std::process::Command) {}

pub fn harden_process() {}

pub fn end_with_parent(_child: &std::process::Child) -> bool {
    true
}

pub fn port_owner(_port: u16) -> Option<u32> {
    None
}

pub fn remove_dirs_after_exit(_dirs: &[std::path::PathBuf]) {}

pub fn show_menu(_entries: Vec<MenuEntry>, _at: Point, _theme: &Theme) -> Option<u32> {
    None
}

pub fn watch_typing(_handler: Box<dyn Fn(super::Typed) + Send + Sync>) -> Result<(), String> {
    Err("Snippet expansion is not available on this platform yet.".into())
}

pub fn stop_typing() {}

pub fn watch_clipboard(_handler: Box<dyn Fn(String) + Send + Sync>) -> Result<(), String> {
    Err("Not supported on this platform.".into())
}

pub fn stop_clipboard_watch() {}

pub fn press(_key: super::Key, _count: usize) -> Result<(), String> {
    Ok(())
}

pub fn insert_slowly(_text: &str) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
}

pub fn uia_selection(_limit: usize) -> Option<super::UiaSelection> {
    None
}

pub fn focused_is_password() -> bool {
    false
}

pub fn input_blocked() -> bool {
    false
}

pub fn clipboard_text() -> Option<String> {
    None
}

/// 1970-01-01 until this platform has a clock: placeholders still work, with that date.
pub fn beep(_volume: u8) {}

pub fn system_uses_dark_theme() -> bool {
    false
}

pub fn float_window(_window: isize) {}

pub fn local_time() -> super::LocalTime {
    super::LocalTime { year: 1970, month: 1, day: 1, weekday: 4, ..Default::default() }
}

pub mod card {
    pub fn init() -> std::io::Result<()> {
        Ok(())
    }
    pub fn show(request: crate::platform::CardRequest) {
        (request.on_closed)();
    }
    pub fn hide() -> bool {
        false
    }
}

pub mod overlay {
    use crate::platform::Theme;

    pub fn init() -> std::io::Result<()> {
        Ok(())
    }
    pub fn show(_id: u64, _top: bool, _theme: Theme) {}
    pub fn hide(_id: u64) {}
    pub fn dismiss() {}
    pub fn caption(_index: usize, _text: &str) {}
    pub fn notice(_text: &str, _top: bool, _theme: Theme) {}
}
