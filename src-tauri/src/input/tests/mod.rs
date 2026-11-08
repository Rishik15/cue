//! File: input/tests/mod.rs
//! Purpose: Tests of input through its public interface and the scripted keyboard: what passes or is swallowed per edge, which actions are active, one final event per recording, and what the app downstream receives.
//! Contents: shared helpers (a three-shortcut test catalog, view readers) and the scenario modules.

use tauri::AppHandle;

use super::fake::Fake;
use super::view::{Event, RecorderView, Subscription, Update};
use super::{Catalog, Key, Mods, Spec, WindowIdentity};

mod chord;
mod keymap;
mod lift;
mod recording;
mod saves;
mod shortcuts;

fn noop(_: &AppHandle) {}
fn noop_hold(_: &AppHandle, _: bool) {}

pub const CA: Mods = Mods(Mods::CTRL_L.0 | Mods::ALT_L.0);

static SPECS: [Spec; 4] = [
    Spec::hold("dictation", "Dictation", (Mods::CTRL_L, Key::Space), noop_hold),
    Spec::press("selection", "Transform Text", (CA, Key::Space), noop),
    Spec::press("grab", "Grab Text", (CA, Key::G), noop),
    Spec::hold("f8", "F8 Hold", (Mods::NONE, Key::F8), noop_hold),
];

pub static CAT: Catalog = Catalog { specs: &SPECS, on_escape: &[], on_key_down: &[] };

/// A keyboard over the test catalog.
pub fn fake() -> Fake {
    Fake::new(&CAT)
}

/// The window the tests record from.
pub fn window() -> WindowIdentity {
    WindowIdentity { label: "settings".into(), incarnation: 1 }
}

/// Every update waiting on the subscription, in order.
pub fn drain(sub: &Subscription<RecorderView>) -> Vec<RecorderView> {
    let mut seen = Vec::new();
    while let Some(update) = sub.try_next() {
        match update {
            Update::Next(v) | Update::Resnapshot(v) => seen.push(v),
        }
    }
    seen
}

/// How many views carry a final event (a cancel or a failure).
pub fn terminals(views: &[RecorderView]) -> usize {
    views.iter().filter(|v| matches!(v.event, Event::Cancelled { .. } | Event::Failed { .. })).count()
}
