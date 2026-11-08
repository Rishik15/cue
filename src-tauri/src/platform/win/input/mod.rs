//! File: win/input/mod.rs
//! Purpose: The Windows adapter of the input module: the one thread that owns the low-level keyboard hook, a button-only mouse hook while a surface needs outside clicks, a hidden session and power watcher, one timer for the nearest deadline, and the marked key sending Cue's own keystrokes use. The reducer behind it (`crate::input`) decides; this only feeds it edges and carries out its answers.
//! Contents: hook — keyboard hook callback; pointer — mouse hook; session — the watcher window; pump — the thread and its message loop; inject — marked SendInput, the held-modifier read and the `Lift` guard other modules use.

mod hook;
mod inject;
mod pointer;
mod pump;
mod session;

pub use inject::{lift_blocked, os_modifiers, send_keys, send_menu_mask, Lift, MARK};
pub use pump::{start_input_pump, wake_input};
