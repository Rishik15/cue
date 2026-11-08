//! File: input/mod.rs
//! Purpose: Input: one module owns the keyboard. A key press is routed once, by one owner, to exactly one of the tray popup, the recorder, a global shortcut or the foreground app; Escape always works while the hook delivers; a stuck or crashed part fails open. Everything outside this folder sees only the names re-exported here.
//! Contents: the public interface (Input, Owner, Recording, PopupLease, Catalog, Spec, views, errors, the injection contract); behind it `core` (the reducer), `route`, `ledger`, `recording`, `popup`, `save`, `commands`, `effects`, `driver` (glue to a platform) and, in tests, `fake` (a scripted keyboard).
//! shortcut: dead_code is allowed because the in-process subscriptions (Subscription, Update), the activity Mark, the lift `send` and the shutdown and recover calls are specified, tested and wired by later features (dictation hold guard, Settings recover button); delete any that is still unused when those land.

#![allow(dead_code)]

mod api;
mod app;
mod bindings;
pub mod bridge;
mod catalog;
mod chord;
mod commands;
mod core;
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
mod tests;

pub use api::{Input, PopupLease, WindowIdentity};
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

#[cfg(test)]
pub use {
    catalog::{Catalog, Spec},
    chord::Chord,
    error::InputError,
    key::Key,
    view::{CatalogView, Event, Phase, Reason, RecorderView, Update},
};
