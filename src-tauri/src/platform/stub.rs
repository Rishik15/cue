//! Purpose: Placeholder backend for non-Windows targets (macOS comes later); every call is a harmless no-op or error.
//! Contents: same surface as `win`: insert, overlay, trim_working_set.

use super::{InsertOptions, MenuEntry, Point, Theme};

pub fn insert(_text: &str, _options: &InsertOptions) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
}

pub fn insert_rich(_text: &str, _html: &str, _options: &InsertOptions) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
}

pub fn open_url(_url: &str) -> Result<(), String> {
    Err("Opening a web page is not available on this platform yet.".into())
}

pub fn grab_region() -> Result<Option<super::Image>, String> {
    Err("Reading text from the screen is not available on this platform yet.".into())
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

pub fn performance_cores() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

pub fn disable_power_throttling() {}

pub fn hide_console(_command: &mut std::process::Command) {}

pub fn harden_process() {}

pub fn end_with_parent(_child: &std::process::Child) {}

pub fn remove_dirs_after_exit(_dirs: &[std::path::PathBuf]) {}

pub fn show_menu(_entries: Vec<MenuEntry>, _at: Point, _theme: &Theme) -> Option<u32> {
    None
}

pub fn watch_typing(_handler: Box<dyn Fn(super::Typed) + Send + Sync>) -> Result<(), String> {
    Err("Snippet expansion is not available on this platform yet.".into())
}

pub fn stop_typing() {}

pub fn press(_key: super::Key, _count: usize) -> Result<(), String> {
    Ok(())
}

pub fn insert_slowly(_text: &str) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
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
pub fn local_time() -> super::LocalTime {
    super::LocalTime { year: 1970, month: 1, day: 1, weekday: 4, ..Default::default() }
}

pub mod overlay {
    use crate::platform::Theme;

    pub fn init() -> std::io::Result<()> {
        Ok(())
    }
    pub fn show(_top: bool, _theme: Theme) {}
    pub fn hide() {}
    pub fn dismiss() {}
    pub fn caption(_index: usize, _text: &str) {}
    pub fn notice(_text: &str, _top: bool, _theme: Theme) {}
}
