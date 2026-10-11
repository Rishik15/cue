//! Purpose: Windows backend for `platform`.
//! Contents: foreground_app name, insert (typing and paste), selection copy, speaker mute, text before the cursor, submit key, clipboard primitives, elevation and IME checks, overlay window, tray popup menu, working-set trim, deferred directory removal, process helpers for the speech worker.

mod app_name;
mod audio_mute;
mod cleanup;
mod clipboard;
mod com;
mod context;
mod elevation;
mod ime;
mod insert;
mod mem;
pub mod overlay;
mod paste;
mod popup;
mod process;
mod selection;
mod shape;
mod theme;

pub use app_name::foreground_app;
pub use audio_mute::{mute_output, unmute_output};
pub use cleanup::remove_dirs_after_exit;
pub use clipboard::set_plain_text as copy_text;
pub use context::text_before_cursor;
pub use insert::{focus_window, foreground_window, insert, press_submit};
pub use mem::trim_working_set;
pub use popup::show_menu;
pub use process::{disable_power_throttling, hide_console, performance_cores};
pub use selection::copy_selection;
