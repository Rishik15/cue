//! File: input/dirty.rs
//! Purpose: The list of hubs the settings window must be told about, owned jointly by the input thread (which adds a hub when it is made) and the effects thread (which drains it every time it wakes). A view reaches the window because its hub is dirty, never because a nudge made it through the bounded queue.
//! Contents: Named / Emit — a view with its window event name, and a hub that can hand out its change as JSON; Dirty — the list and its `add`, `drain`.

use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::view::{CatalogView, Hub, RecorderView};

/// A view that is sent to the window under one event name.
pub trait Named {
    /// The window event the view is emitted as.
    const EVENT: &'static str;
}

impl Named for RecorderView {
    const EVENT: &'static str = "shortcut-recorder";
}

impl Named for CatalogView {
    const EVENT: &'static str = "shortcut-catalog";
}

/// A hub that can hand out what changed since it was last drained.
pub trait Emit: Send + Sync {
    /// The event name and the JSON of the latest view when it is newer than the last one handed out.
    fn take(&self) -> Option<(&'static str, serde_json::Value)>;

    /// Whether a change waits to be taken.
    fn pending(&self) -> bool;
}

impl<T: Clone + Serialize + Named + Send + 'static> Emit for Hub<T> {
    /// The latest view as JSON when it changed since the last take.
    fn take(&self) -> Option<(&'static str, serde_json::Value)> {
        let view = self.take_dirty()?;
        serde_json::to_value(view).ok().map(|value| (T::EVENT, value))
    }

    /// Whether the hub changed since the last take.
    fn pending(&self) -> bool {
        self.is_dirty()
    }
}

/// The hubs to drain; a hub nobody else holds and with nothing pending is dropped.
#[derive(Default)]
pub struct Dirty {
    hubs: Mutex<Vec<Arc<dyn Emit>>>,
}

impl Dirty {
    /// An empty list.
    pub fn new() -> Arc<Dirty> {
        Arc::new(Dirty::default())
    }

    /// Adds a hub to drain. Takes the lock only to push, so the input thread may call it.
    pub fn add(&self, hub: Arc<dyn Emit>) {
        self.hubs.lock().unwrap_or_else(|e| e.into_inner()).push(hub);
    }

    /// Hands every changed hub's latest view to `send`, in the order the hubs were added, then forgets the hubs no one else holds and nothing waits on. Run by the effects thread each time it wakes; `send` runs with no lock held.
    pub fn drain(&self, mut send: impl FnMut(&'static str, serde_json::Value)) {
        let hubs: Vec<Arc<dyn Emit>> = self.hubs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        for hub in &hubs {
            if let Some((event, value)) = hub.take() {
                send(event, value);
            }
        }
        drop(hubs);
        self.hubs.lock().unwrap_or_else(|e| e.into_inner()).retain(|h| Arc::strong_count(h) > 1 || h.pending());
    }
}
