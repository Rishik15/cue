//! Purpose: Windows backend for `platform`.
//! Contents: watch_typing / stop_typing — the keyboard watcher for snippet expansion; watch_clipboard / stop_clipboard_watch — copied text, for a note collecting it; local_time; beep; float_window; foreground_app name, insert (typing and paste), selection copy, speaker mute, text before the cursor, submit key, clipboard primitives, elevation and IME checks, overlay window, tray popup menu, working-set trim, deferred directory removal, process helpers for the speech worker, zip unpacking, the pointer and monitor work area.

mod app_ids;
mod app_name;
mod archive;
mod audio_mute;
mod beep;
pub mod card;
mod cf_html;
mod cleanup;
mod clip_watch;
mod clipboard;
mod clock;
mod com;
mod context;
mod elevation;
mod fonts;
mod grab;
mod ime;
mod input;
mod insert;
mod installed_apps;
mod mem;
mod monitor;
mod open;
pub mod overlay;
mod palette;
mod paste;
mod popup;
mod port_owner;
mod process;
mod screen;
mod selection;
mod shape;
mod theme;
mod topmost;
mod typing;

pub use app_name::{foreground_app, list_apps};
pub use archive::extract_zip;
pub use audio_mute::{mute_output, unmute_output};
pub use beep::beep;
pub use cleanup::remove_dirs_after_exit;
pub use clip_watch::{stop as stop_clipboard_watch, watch as watch_clipboard};
pub use clipboard::set_plain_text as copy_text;
pub use clipboard::unicode_text as clipboard_text;
pub use clock::local_time;
pub use context::{focused_is_password, text_before_cursor, uia_selection};
pub use grab::{
    grab_region, ocr::languages as ocr_languages, ocr::recognize as recognize_text, ocr::recognize_file as recognize_image_file,
};
pub use insert::{focus_window, foreground_window, insert, insert_rich, insert_slowly, press, press_submit, replace};
pub use mem::trim_working_set;
pub use monitor::work_area_of;
pub use open::{open_url, start_program};
pub use popup::show_menu;
pub use port_owner::port_owner;
pub use process::{disable_power_throttling, end_with_parent, harden_process, hide_console, performance_cores};
pub use screen::selection_anchor;
pub use selection::copy_selection;
pub use theme::system_uses_dark_theme;
pub use topmost::float_window;
pub use typing::{stop as stop_typing, watch as watch_typing};

/// Whether the focused app is one Cue's input cannot reach (it runs as administrator while Cue does not).
pub fn input_blocked() -> bool {
    elevation::target_blocks_input()
}
