//! File: input/mod.rs
//! Purpose: Input: one module owns the keyboard. A key press is routed once, by one owner, to exactly one of the tray popup, the recorder, a global shortcut or the foreground app; Escape always works while the hook delivers; a stuck or crashed part fails open. Everything outside this folder sees only the names re-exported here.
//! Contents: the public interface re-exported here (Input, PopupLease, the catalog, Edge / Origin / Verdict, Runtime and Sink for a platform, Mods, the key map, lift and the activity mark); behind it `api` and `bridge` (handles and the settings window's commands), `app` (start inside Cue), `core` with `route`, `ledger`, `gesture`, `recording`, `popup`, `publish`, `commands`, `bindings` and `save` (the reducer and its parts), `effects`, `driver` (glue to a platform), `key`, `chord`, `keymap`, `lift`, `shared`, `view`, `health`, `error`, `mailbox`, `catalog`, and in tests `fake` / `fake_parts` (a scripted keyboard) and `tests`.

mod api;
mod app;
mod bindings;
pub mod bridge;
mod catalog;
mod chord;
mod commands;
mod core;
mod dirty;
mod driver;
mod effects;
mod error;
mod gesture;
mod health;
mod key;
mod keymap;
mod ledger;
mod lift;
mod mailbox;
mod popup;
mod publish;
mod recording;
mod route;
mod save;
mod shared;
mod view;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod fake_parts;
#[cfg(test)]
mod tests;

pub use api::{Input, PopupLease};
pub use app::{refresh_enabled, start};
pub use bridge::{window_closed, Bridge};
pub use catalog::CATALOG;
pub use core::{Edge, Origin, Verdict};
pub use driver::{ArmOutcome, Runtime};
pub use effects::Sink;
pub use health::Health;
pub use key::Mods;
pub use keymap::{is_phantom_ctrl, slot};
pub use lift::{lift, KeyBatch, KeyStep, LiftGuard};
pub use popup::{PopupKey, Rect};
pub use shared::{activity_mark, Mark};

#[cfg(test)]
pub use {
    catalog::{Catalog, Spec},
    chord::Chord,
    error::InputError,
    key::Key,
    view::{CatalogView, Event, Phase, Reason, RecorderView, Update},
};
