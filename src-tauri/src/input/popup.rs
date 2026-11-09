//! File: input/popup.rs
//! Purpose: The tray popup as a surface of the input module: it claims Escape, Enter, Up and Down while it is open (highest priority), asks to close on window-switching keys and outside clicks, and is revoked when its inbox is gone. It has no hooks of its own.
//! Contents: PopupKey — what the popup is told; Rect — a physical-pixel rectangle; Popup — the open lease; the Core methods that claim, follow and revoke it.

use std::time::{Duration, Instant};

use super::core::{Core, Edge};
use super::key::{Key, Mods};
use super::ledger::{Owner, Slot};
use super::view::{Event, Reason};

/// What the input module tells an open popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupKey {
    Escape,
    Enter,
    Up,
    Down,
    Dismiss,
}

/// A rectangle in physical screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    /// Whether the point lies inside.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// How long a popup may own the menu keys without hearing from them: a menu whose window stalls cannot hold Escape, Enter and the arrows longer than this. Every key told to it starts the time again.
pub const POPUP_STALL: Duration = Duration::from_secs(30);

/// An open popup: where it is, when it is given up on, and how to tell it things. `inbox` returns false when the popup is gone.
pub struct Popup {
    pub id: u64,
    pub deadline: Instant,
    pub bounds: Rect,
    pub inbox: Box<dyn Fn(PopupKey) -> bool + Send>,
}

/// The popup key a main key means, if it is one of the four the popup owns.
fn popup_key(key: Key) -> Option<PopupKey> {
    match key {
        Key::Escape => Some(PopupKey::Escape),
        Key::Return | Key::KeypadEnter => Some(PopupKey::Enter),
        Key::UpArrow => Some(PopupKey::Up),
        Key::DownArrow => Some(PopupKey::Down),
        _ => None,
    }
}

/// Whether the edge switches windows (Alt, Win, Tab), which closes the popup.
fn switches_window(slot: Slot) -> bool {
    match slot {
        Slot::Key(k) => k == Key::Tab,
        Slot::Mod(m) => m.any(Mods::ALT_L | Mods::ALT_R | Mods::WIN_L | Mods::WIN_R),
    }
}

impl Core {
    /// Sends `key` to the popup; false (and the popup is revoked) when its inbox is gone.
    fn tell_popup(&mut self, key: PopupKey) -> bool {
        let now = self.now;
        let alive = self.popup.as_mut().is_some_and(|p| {
            p.deadline = now + POPUP_STALL;
            (p.inbox)(key)
        });
        if !alive {
            self.revoke_popup();
        }
        alive
    }

    /// Whether `e` is a down the menu must hear although the ledger already counts the key as down: a key an earlier menu was given and never saw released, or an Escape held while the menu opened. Another held key (an arrow) keeps its first owner, so the app still gets its release.
    pub(super) fn popup_key_is_stale(&self, e: Edge, owner: Owner) -> bool {
        let Some(p) = self.popup.as_ref() else { return false };
        let Slot::Key(k) = e.slot else { return false };
        match owner {
            Owner::Popup(id) => popup_key(k).is_some() && id != p.id,
            Owner::Pass => k == Key::Escape,
            _ => false,
        }
    }

    /// Asks the popup to close and then revokes it, so no window stays on screen without an input lease.
    pub(super) fn dismiss_popup(&mut self) {
        if self.popup.is_some() {
            self.tell_popup(PopupKey::Dismiss);
            self.revoke_popup();
        }
    }

    /// Claims a fresh press for the popup when it is one of its keys; Alt, Win and Tab ask it to close and then pass on.
    pub(super) fn popup_claim(&mut self, e: Edge) -> Option<Owner> {
        let id = self.popup.as_ref()?.id;
        if let Slot::Key(k) = e.slot {
            if let Some(key) = popup_key(k) {
                return self.tell_popup(key).then_some(Owner::Popup(id));
            }
        }
        if switches_window(e.slot) {
            self.tell_popup(PopupKey::Dismiss);
        }
        None
    }

    /// A repeat of a popup key navigates again; a release changes nothing.
    pub(super) fn popup_follow(&mut self, id: u64, e: Edge) {
        let same = self.popup.as_ref().is_some_and(|p| p.id == id);
        if let (true, true, Slot::Key(k)) = (e.down, same, e.slot) {
            if let Some(key) = popup_key(k) {
                self.tell_popup(key);
            }
        }
    }

    /// Removes the popup; its held keys keep being swallowed until released.
    pub(super) fn revoke_popup(&mut self) {
        if let Some(p) = self.popup.take() {
            self.ledger.reassign(Owner::Popup(p.id), Owner::Drain);
        }
    }

    /// Opens a popup, ending a recording first and replacing an earlier popup.
    pub(super) fn open_popup(&mut self, popup: Popup, now: Instant) {
        self.end_lease(Event::Cancelled { reason: Reason::Replaced }, false, now);
        self.dismiss_popup();
        self.popup = Some(Popup { deadline: now + POPUP_STALL, ..popup });
    }

    /// Closes popup `id` if it is the open one.
    pub(super) fn close_popup(&mut self, id: u64) {
        if self.popup.as_ref().is_some_and(|p| p.id == id) {
            self.revoke_popup();
        }
    }

    /// A pointer button went down at (x, y): outside an open popup it asks the popup to close, outside the recording's regions it cancels the recording.
    pub fn pointer_down(&mut self, x: i32, y: i32, now: Instant) {
        if self.popup.as_ref().is_some_and(|p| !p.bounds.contains(x, y)) {
            self.tell_popup(PopupKey::Dismiss);
        }
        let outside = self.lease.as_ref().is_some_and(|l| !l.regions.is_empty() && !l.regions.iter().any(|r| r.contains(x, y)));
        if outside {
            self.end_lease(Event::Cancelled { reason: Reason::Outside }, false, now);
        }
    }

    /// The popup is given up on when it has been silent too long: it is asked to close and then revoked, so its keys pass again.
    pub(super) fn expire_popup(&mut self, now: Instant) {
        if self.popup.as_ref().is_some_and(|p| now >= p.deadline) {
            self.tell_popup(PopupKey::Dismiss);
            self.revoke_popup();
        }
    }

    /// Whether a surface needs to hear pointer buttons, so the driver installs the mouse hook only then.
    pub fn needs_pointer(&self) -> bool {
        self.popup.is_some() || self.lease.as_ref().is_some_and(|l| !l.regions.is_empty())
    }
}
