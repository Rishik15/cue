//! File: input/core.rs
//! Purpose: The reducer behind the keyboard: one owner of every physical edge. It routes each press once, to the tray popup, the recorder, a global shortcut or nobody, and its repeats and release follow. It never touches the OS, the UI or a feature; it only returns pass or swallow and queues effects, so the same code runs under the Windows hook and under the scripted fake.
//! Contents: Origin / Edge / Verdict — an edge in, a decision out; Core — all state, with `edge`, `session`, `overflow`, `tick` and the views. Commands live in `commands`, the recording lease in `recording`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use super::bindings::load_bindings;
use super::catalog::Catalog;
use super::commands::Command;
use super::effects::{Effect, Gens};
use super::health::Health;
use super::key::{Key, Mods};
use super::ledger::{Ledger, Owner, Slot};
use super::mailbox::Mailbox;
use super::popup::Popup;
use super::recording::Lease;
use super::route::{Binding, Change, Matcher};
use super::save::{Pending, Store, Writer};
use super::shared::Shared;
use super::view::{CatalogView, Hub};

/// Modifiers whose lone press and release opens a shell menu (Start, the menu bar).
const MENU_MODS: Mods = Mods(Mods::ALT_L.0 | Mods::ALT_R.0 | Mods::WIN_L.0 | Mods::WIN_R.0);

/// Where an edge came from. Cue's own marked keys and the AltGr phantom Ctrl are invisible to everything; any other key, injected by another program or not, counts as physical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Physical,
    Cue,
    Phantom,
}

/// One key or modifier going down (a repeat is a down of a key already down) or up.
#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub slot: Slot,
    pub down: bool,
    pub origin: Origin,
}

/// What the hook returns for the edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Swallow,
}

/// The whole input state. Owned by the input thread; nothing in it is shared.
pub struct Core {
    pub(super) catalog: &'static Catalog,
    pub(super) store: Arc<dyn Store>,
    pub(super) mailbox: Arc<Mailbox<Command>>,
    pub(super) shared: Arc<Shared>,
    pub(super) gens: Arc<Gens>,
    pub(super) hub: Arc<Hub<CatalogView>>,
    pub(super) writer: Writer,
    pub(super) ledger: Ledger,
    pub(super) bindings: Vec<Binding>,
    pub(super) errors: Vec<Option<String>>,
    pub(super) matcher: Matcher,
    pub(super) pending: Pending,
    pub(super) lease: Option<Lease>,
    pub(super) popup: Option<Popup>,
    pub(super) revoked: HashMap<u64, u64>,
    pub(super) epochs: u64,
    pub(super) tickets: u64,
    pub(super) revision: u64,
    pub(super) health: Health,
    pub(super) last_error: Option<String>,
    pub(super) arm_request: Option<u64>,
    pub(super) now: Instant,
    pub(super) mask_sent: bool,
    pub out: Vec<Effect>,
}

impl Core {
    /// A core over the stored bindings. Rows that fail whole-set validation start inactive with a message.
    pub fn new(catalog: &'static Catalog, store: Arc<dyn Store>, mailbox: Arc<Mailbox<Command>>, shared: Arc<Shared>) -> Core {
        let (bindings, errors) = load_bindings(catalog, store.as_ref());
        let committed = bindings.iter().map(|b| b.valid.then_some(b.chord)).collect();
        let n = bindings.len();
        let hub = Hub::new(CatalogView { revision: 0, shortcuts: Vec::new(), health: Health::InstalledUnproven, last_error: None });
        let mut core = Core {
            catalog,
            store,
            mailbox,
            shared,
            gens: Gens::new(n),
            hub,
            writer: Writer::new(),
            ledger: Ledger::new(),
            bindings,
            errors,
            matcher: Matcher::new(n),
            pending: Pending::new(committed),
            lease: None,
            popup: None,
            revoked: HashMap::new(),
            epochs: 0,
            tickets: 0,
            revision: 0,
            health: Health::InstalledUnproven,
            last_error: None,
            arm_request: None,
            now: Instant::now(),
            mask_sent: false,
            out: Vec::new(),
        };
        core.publish_catalog();
        core
    }

    /// Takes one edge and says whether the keyboard passes it on.
    pub fn edge(&mut self, e: Edge, now: Instant) -> Verdict {
        self.now = now;
        if e.origin != Origin::Physical {
            return Verdict::Pass;
        }
        self.shared.bump_activity();
        if self.health != Health::Observed && self.health != Health::Failed {
            self.health = self.health.after_key();
            self.publish_catalog();
        }
        let was_down = self.ledger.is_down(e.slot);
        let owner = if e.down && !was_down {
            self.ledger.press(e.slot, Owner::Pass);
            self.shared.set_held(self.ledger.held_mods());
            let owner = self.claim(e);
            self.ledger.press(e.slot, owner);
            owner
        } else if e.down {
            let owner = self.ledger.owner(e.slot);
            self.follow(e, owner, true);
            owner
        } else {
            let owner = self.ledger.release(e.slot);
            self.shared.set_held(self.ledger.held_mods());
            self.mask_sent &= self.ledger.held_mods().any(MENU_MODS);
            self.follow(e, owner, false);
            owner
        };
        if owner.captured() {
            Verdict::Swallow
        } else {
            Verdict::Pass
        }
    }

    /// Decides who a fresh press belongs to: the popup for its keys, the armed recorder for everything, else a matching shortcut or nobody.
    fn claim(&mut self, e: Edge) -> Owner {
        if let Some(owner) = self.popup_claim(e) {
            return owner;
        }
        if let Some(epoch) = self.recorder_epoch() {
            self.feed_recorder(epoch, e, false);
            self.mask_if_needed();
            return Owner::Recorder(epoch);
        }
        self.observe(e);
        let changes = self.step_matcher(e);
        let key = match e.slot {
            Slot::Key(k) => Some(k),
            Slot::Mod(_) => None,
        };
        let keyed = changes.iter().find(|(i, down)| *down && key.is_some() && self.bindings[*i as usize].chord.key == key);
        let owner = keyed.map_or(Owner::Pass, |(i, _)| Owner::Action(*i));
        self.emit_actions(&changes);
        if owner.captured() {
            self.mask_if_needed();
        }
        owner
    }

    /// A key is about to be swallowed while Alt or Win, whose press already reached the shell, is held: one inert key tap is queued so letting go of it does not open a menu. Once per hold.
    fn mask_if_needed(&mut self) {
        if !self.mask_sent && self.ledger.passed_mods().any(MENU_MODS) {
            self.mask_sent = true;
            self.out.push(Effect::MenuMask);
        }
    }

    /// Passes a repeat or a release to whoever owns the press.
    fn follow(&mut self, e: Edge, owner: Owner, repeat: bool) {
        match owner {
            Owner::Popup(id) => self.popup_follow(id, e),
            Owner::Recorder(epoch) => self.feed_recorder(epoch, e, repeat),
            Owner::Drain => {}
            Owner::Pass | Owner::Action(_) => {
                if let (Some(epoch), Slot::Mod(_)) = (self.recorder_epoch(), e.slot) {
                    self.feed_recorder(epoch, e, repeat);
                    return;
                }
                let changes = self.step_matcher(e);
                self.emit_actions(&changes);
            }
        }
    }

    /// Runs the shortcut matcher for an edge, unless a recording holds the keyboard.
    fn step_matcher(&mut self, e: Edge) -> Vec<Change> {
        let mut changes = Vec::new();
        if self.lease.is_some() {
            return changes;
        }
        let key = match e.slot {
            Slot::Key(k) => Some(k),
            Slot::Mod(_) => None,
        };
        self.matcher.step(&self.bindings, self.ledger.held_mods(), key, e.down, &mut changes);
        changes
    }

    /// Signals the features that watch Escape and typing without owning a shortcut.
    fn observe(&mut self, e: Edge) {
        let Slot::Key(key) = e.slot else { return };
        if key == Key::Escape {
            self.out.push(Effect::Escape);
        }
        if !self.bindings.iter().any(|b| b.active().is_some_and(|h| h.key == Some(key))) {
            self.out.push(Effect::Typing);
        }
    }

    /// The key state may be wrong (lock, resume, a new desktop): every action is released, every press forgotten, and surfaces that depend on keys end.
    pub fn session(&mut self, now: Instant) {
        self.now = now;
        self.shared.new_epoch();
        self.retire_all();
        self.ledger.forget_all();
        self.shared.set_held(Mods::NONE);
        self.end_lease(super::view::Event::Cancelled { reason: super::view::Reason::Interrupted }, false, now);
    }

    /// An effect could not be queued: nothing is swallowed any more, every action is released and every surface ends, so the keyboard is never left captured.
    pub fn overflow(&mut self, now: Instant) {
        self.now = now;
        self.ledger.uncapture_all();
        self.retire_all();
        self.end_lease(super::view::Event::Failed { message: super::error::InputError::Overloaded.to_string() }, true, now);
        self.revoke_popup();
    }

    /// Sets the hook state shown to the user.
    pub fn set_health(&mut self, health: Health) {
        if self.health != health {
            self.health = health;
            self.publish_catalog();
        }
    }
}
