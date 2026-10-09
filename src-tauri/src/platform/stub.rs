//! Purpose: Placeholder backend for non-Windows targets (macOS comes later); every call is a harmless no-op or error.
//! Contents: same surface as `win`: insert, overlay, trim_working_set.

use super::{InsertOptions, MenuEntry, Point};

pub fn insert(_text: &str, _options: &InsertOptions) -> Result<(), String> {
    Err("Text insertion is not available on this platform yet.".into())
}

pub fn trim_working_set() {}

pub fn remove_dirs_after_exit(_dirs: &[std::path::PathBuf]) {}

pub fn show_menu(_entries: Vec<MenuEntry>, _at: Point) -> Option<u32> {
    None
}

pub mod overlay {
    pub fn init() -> std::io::Result<()> {
        Ok(())
    }
    pub fn show(_top: bool) {}
    pub fn hide() {}
}
