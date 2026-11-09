//! File: input/publish.rs
//! Purpose: What the core tells the outside: the shortcut list view and its hub, the generations the effects side checks, and the action edges (with the generation bump that drops a stale start and sends the release first).
//! Contents: the Core methods catalog_hub, dirty, gens, catalog_view, publish_catalog, emit_actions, retire_action and retire_all.

use std::sync::Arc;

use super::core::Core;
use super::dirty::Dirty;
use super::effects::{Effect, Gens};
use super::route::Change;
use super::view::{CatalogView, Hub, ShortcutView};

impl Core {
    /// The hub the shortcut list is published on.
    pub fn catalog_hub(&self) -> Arc<Hub<CatalogView>> {
        self.hub.clone()
    }

    /// The hubs the effects side drains each time it wakes.
    pub fn dirty(&self) -> Arc<Dirty> {
        self.dirty.clone()
    }

    /// The generations the effects side checks.
    pub fn gens(&self) -> Arc<Gens> {
        self.gens.clone()
    }

    /// The shortcut list as it is now.
    pub fn catalog_view(&self) -> CatalogView {
        let shortcuts = self.catalog.specs.iter().enumerate().map(|(i, spec)| ShortcutView {
            id: spec.id,
            label: spec.label,
            parts: self.bindings[i].chord.parts(),
            enabled: self.bindings[i].enabled,
            held: self.matcher.is_down(i),
            error: self.errors[i].clone(),
        });
        CatalogView { revision: self.revision, shortcuts: shortcuts.collect(), health: self.health, last_error: self.last_error.clone() }
    }

    /// Publishes the shortcut list.
    pub(super) fn publish_catalog(&mut self) {
        let view = self.catalog_view();
        self.hub.publish(view);
        self.out.push(Effect::Views);
    }

    /// Queues the action edges `changes` with each shortcut's current generation.
    pub(super) fn emit_actions(&mut self, changes: &[Change]) {
        for &(idx, down) in changes {
            let gen = self.gens.get(idx as usize);
            self.out.push(Effect::Action { idx, down, gen });
        }
        if !changes.is_empty() {
            self.publish_catalog();
        }
    }

    /// Bumps the generation of shortcut `idx` and lets go of it if it is down, so a start still queued is dropped and the release is sent.
    pub(super) fn retire_action(&mut self, idx: usize) {
        self.gens.bump(idx);
        let mut changes = Vec::new();
        self.matcher.release(idx, &mut changes);
        self.emit_actions(&changes);
    }

    /// Retires every shortcut.
    pub(super) fn retire_all(&mut self) {
        for idx in 0..self.bindings.len() {
            self.retire_action(idx);
        }
    }
}
