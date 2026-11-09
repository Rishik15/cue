//! File: input/api.rs
//! Purpose: The public face of input for everything outside this module: start-independent handles that queue requests for the input thread and read published views. Nothing here touches the keyboard.
//! Contents: Input — catalog, reset, refresh, recover, owner, shutdown; Owner — one window's handle (record, tray popup); Recording — one recording (snapshot, regions, cancel); PopupLease — an open tray popup.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::catalog::Catalog;
use super::commands::Command;
use super::error::InputError;
use super::mailbox::Mailbox;
use super::popup::{Popup, PopupKey, Rect};
#[cfg(test)]
use super::view::Subscription;
use super::view::{CatalogView, Completion, Hub, Reason, RecorderView};

/// What `Input::shutdown` runs to end the input thread.
pub type Stop = Box<dyn FnOnce() + Send>;

pub(super) struct Inner {
    pub(super) mailbox: Arc<Mailbox<Command>>,
    pub(super) hub: Arc<Hub<CatalogView>>,
    pub(super) catalog: &'static Catalog,
    pub(super) ids: AtomicU64,
}

/// The input module as other code sees it.
pub struct Input {
    pub(super) inner: Arc<Inner>,
    stop: Mutex<Option<Stop>>,
}

impl Input {
    /// Handles over a running input thread; `stop` ends it.
    pub(super) fn new(mailbox: Arc<Mailbox<Command>>, hub: Arc<Hub<CatalogView>>, catalog: &'static Catalog, stop: Stop) -> Input {
        Input { inner: Arc::new(Inner { mailbox, hub, catalog, ids: AtomicU64::new(1) }), stop: Mutex::new(Some(stop)) }
    }

    /// The shortcut list as it is now.
    pub fn catalog(&self) -> CatalogView {
        self.inner.hub.latest()
    }

    /// A handle that can wait, off the input thread, for the list to reach a revision.
    pub fn reader(&self) -> CatalogReader {
        CatalogReader(self.inner.hub.clone())
    }

    /// The shortcut list: a snapshot first, then each revision.
    #[cfg(test)]
    pub fn shortcuts(&self) -> Subscription<CatalogView> {
        self.inner.hub.subscribe()
    }

    /// Puts a shortcut back to its default. The answer is the new revision, once it is saved.
    pub fn reset(&self, id: &str) -> Completion<Result<u64, InputError>> {
        let (tx, completion) = Completion::pair();
        match self.inner.catalog.specs.iter().position(|s| s.id == id) {
            Some(idx) => self.inner.mailbox.post(Command::Reset { idx, reply: tx }),
            None => {
                let _ = tx.send(Err(InputError::UnknownShortcut(id.into())));
            }
        }
        completion
    }

    /// Re-reads the on/off switches of every feature and shortcut.
    pub fn refresh_enabled(&self) {
        self.inner.mailbox.post(Command::RefreshEnabled);
    }

    /// Releases every action and forgets key state, as after a session change.
    pub fn recover(&self) -> Completion<Result<(), InputError>> {
        let (tx, completion) = Completion::pair();
        self.inner.mailbox.post(Command::Recover { reply: tx });
        completion
    }

    /// The handle for one window; dropping it ends whatever the window had open.
    pub fn owner(&self) -> Owner {
        Owner { id: self.inner.ids.fetch_add(1, Ordering::AcqRel), seq: AtomicU64::new(0), inner: self.inner.clone() }
    }

    /// Ends the input thread; its hooks and window are removed on the thread itself. A second call does nothing.
    pub fn shutdown(&self) {
        if let Some(stop) = self.stop.lock().unwrap_or_else(|e| e.into_inner()).take() {
            stop();
        }
    }
}

/// A handle on the published shortcut list.
pub struct CatalogReader(Arc<Hub<CatalogView>>);

impl CatalogReader {
    /// The list once it reaches `revision` (a change is acknowledged before its list is published), or the latest after `within`.
    pub fn at(&self, revision: u64, within: std::time::Duration) -> CatalogView {
        let end = std::time::Instant::now() + within;
        loop {
            let view = self.0.latest();
            if view.revision >= revision || std::time::Instant::now() >= end {
                return view;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

/// One window's handle.
pub struct Owner {
    pub(super) id: u64,
    seq: AtomicU64,
    inner: Arc<Inner>,
}

impl Owner {
    /// Starts recording shortcut `id`. Returns at once with the recording in its Starting phase; the request number is taken now, so a cancel is valid in every phase.
    pub fn record(&self, id: &str) -> Result<Recording, InputError> {
        let idx = self.inner.catalog.specs.iter().position(|s| s.id == id).ok_or_else(|| InputError::UnknownShortcut(id.into()))?;
        let seq = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
        let hub = Hub::new(RecorderView {
            lease: seq,
            seq: 0,
            phase: super::view::Phase::Starting,
            open: true,
            parts: Vec::new(),
            live: false,
            held: false,
            error: None,
            saves: 0,
            revision: 0,
            event: super::view::Event::None,
        });
        self.inner.mailbox.post(Command::Record { owner: self.id, seq, idx, hub: hub.clone() });
        Ok(Recording { owner: self.id, seq, hub, inner: self.inner.clone() })
    }

    /// Opens the tray popup at `bounds`. `inbox` is called on the effects side with each key and returns false when the popup is gone, which revokes the lease.
    pub fn tray_popup(&self, bounds: Rect, inbox: impl Fn(PopupKey) -> bool + Send + 'static) -> Result<PopupLease, InputError> {
        let id = self.inner.ids.fetch_add(1, Ordering::AcqRel);
        self.inner.mailbox.post(Command::OpenPopup(Popup { id, deadline: std::time::Instant::now(), bounds, inbox: Box::new(inbox) }));
        Ok(PopupLease { id, inner: self.inner.clone() })
    }
}

impl Drop for Owner {
    /// Revokes every recording of the window.
    fn drop(&mut self) {
        self.inner.mailbox.post(Command::OwnerGone { owner: self.id });
    }
}

/// One recording.
pub struct Recording {
    owner: u64,
    seq: u64,
    hub: Arc<Hub<RecorderView>>,
    inner: Arc<Inner>,
}

impl Recording {
    /// The recording's view as it is now.
    pub fn snapshot(&self) -> RecorderView {
        self.hub.latest()
    }

    /// The recording's views: a snapshot first, then each change in order; a gap gives a fresh snapshot.
    #[cfg(test)]
    pub fn observe(&self) -> Subscription<RecorderView> {
        self.hub.subscribe()
    }

    /// Tells the recording where the control and its popover are, in physical pixels; a click outside them cancels.
    pub fn regions(&self, rects: Vec<Rect>) {
        self.inner.mailbox.post(Command::Regions { owner: self.owner, seq: self.seq, rects });
    }

    /// Ends the recording now. Never waits for a save; the answer says the input thread handled it.
    pub fn cancel(&self) -> Completion<()> {
        let (tx, receipt) = Completion::pair();
        self.inner.mailbox.post(Command::Cancel { owner: self.owner, seq: self.seq, reason: Reason::Outside, reply: Some(tx) });
        receipt
    }
}

impl Drop for Recording {
    /// A recording dropped with its page revokes itself.
    fn drop(&mut self) {
        self.inner.mailbox.post(Command::Cancel { owner: self.owner, seq: self.seq, reason: Reason::WindowClosed, reply: None });
    }
}

/// The open tray popup; dropping it closes the lease.
pub struct PopupLease {
    id: u64,
    inner: Arc<Inner>,
}

impl Drop for PopupLease {
    /// Closes the popup lease.
    fn drop(&mut self) {
        self.inner.mailbox.post(Command::ClosePopup { id: self.id });
    }
}
