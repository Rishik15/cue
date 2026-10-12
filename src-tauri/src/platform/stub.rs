//! Purpose: Placeholder backend for non-Windows targets (macOS comes later); every call is a harmless no-op or error.
//! Contents: same surface as `win`: insert, overlay, trim_working_set.

use super::{InsertOptions, MenuEntry, Point, Theme};

pub fn insert(_text: &str, _options: &InsertOptions) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
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

pub fn copy_selection() -> Result<Option<String>, String> {
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

pub mod overlay {
    use crate::platform::Theme;

    pub fn init() -> std::io::Result<()> {
        Ok(())
    }
    pub fn show(_top: bool, _theme: Theme) {}
    pub fn hide() {}
    pub fn dismiss() {}
    pub fn caption(_index: usize, _text: &str) {}
    pub fn caption_after(_after: Option<std::time::Duration>) {}
    pub fn notice(_text: &str, _top: bool, _theme: Theme) {}
}
