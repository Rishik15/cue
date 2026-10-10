//! Purpose: Placeholder backend for non-Windows targets (macOS comes later); every call is a harmless no-op or error.
//! Contents: same surface as `win`: insert, overlay, trim_working_set.

use super::{InsertOptions, MenuEntry, Point, Theme};

pub fn insert(_text: &str, _options: &InsertOptions) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
}

pub fn foreground_window() -> isize {
    0
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

pub fn remove_dirs_after_exit(_dirs: &[std::path::PathBuf]) {}

pub fn show_menu(_entries: Vec<MenuEntry>, _at: Point) -> Option<u32> {
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
    pub fn notice(_text: &str, _top: bool, _theme: Theme) {}
}
