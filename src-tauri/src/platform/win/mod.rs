//! Purpose: Windows backend for `platform`.
//! Contents: insert (typing and paste), clipboard primitives, elevation check, overlay window, tray popup menu, working-set trim, deferred directory removal.

mod cleanup;
mod clipboard;
mod elevation;
mod insert;
mod mem;
pub mod overlay;
mod paste;
mod popup;
mod shape;
mod theme;

pub use cleanup::remove_dirs_after_exit;
pub use insert::insert;
pub use mem::trim_working_set;
pub use popup::show_menu;
