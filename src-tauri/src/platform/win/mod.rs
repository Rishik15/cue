//! Purpose: Windows backend for `platform`.
//! Contents: insert (typing and paste), clipboard primitives, elevation check, overlay window, tray popup menu, working-set trim, deferred directory removal, process helpers for the speech worker.

mod cleanup;
mod clipboard;
mod elevation;
mod insert;
mod mem;
pub mod overlay;
mod paste;
mod popup;
mod process;
mod shape;
mod theme;

pub use cleanup::remove_dirs_after_exit;
pub use clipboard::set_plain_text as copy_text;
pub use insert::{foreground_window, insert};
pub use mem::trim_working_set;
pub use popup::show_menu;
pub use process::{disable_power_throttling, hide_console, performance_cores};
