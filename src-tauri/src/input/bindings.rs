//! File: input/bindings.rs
//! Purpose: Loads the stored shortcuts at startup and checks them as a set. A row that does not parse, is unusable, or clashes with an earlier row stays inactive with a visible message and is never overwritten.
//! Contents: load_bindings — every shortcut's binding and its problem, if any.

use super::catalog::Catalog;
use super::chord::Chord;
use super::key::{Key, Mods};
use super::route::Binding;
use super::save::Store;

/// The stored bindings of every shortcut, each with the problem that keeps it inactive, if any.
fn read(catalog: &Catalog, store: &dyn Store) -> (Vec<Binding>, Vec<Option<String>>) {
    let mut bindings = Vec::new();
    let mut errors = Vec::new();
    for spec in catalog.specs {
        let default = Chord::new(spec.default.0, Some(spec.default.1)).unwrap_or(Chord {
            modifiers: Mods::NONE,
            key: Some(Key::Space),
            both: Mods::NONE,
        });
        let (chord, error) = match store.load(spec.id) {
            None => (default, None),
            Some(value) => match serde_json::from_value::<Chord>(value) {
                Ok(c) => (c, None),
                Err(e) => (default, Some(format!("The saved keys for {} could not be read ({e}). Reset it to fix this.", spec.label))),
            },
        };
        bindings.push(Binding { chord, valid: error.is_none(), enabled: store.enabled(spec.id) });
        errors.push(error);
    }
    (bindings, errors)
}

/// Marks every stored row that is unusable or clashes with an earlier row as inactive, with a message.
fn check_set(catalog: &Catalog, bindings: &mut [Binding], errors: &mut [Option<String>]) {
    for i in 0..bindings.len() {
        if !bindings[i].valid {
            continue;
        }
        let others: Vec<Option<Chord>> = bindings.iter().enumerate().map(|(j, b)| (j < i && b.valid).then_some(b.chord)).collect();
        if let Err(e) = super::save::validate(catalog.specs, &others, i, &bindings[i].chord) {
            bindings[i].valid = false;
            errors[i] = Some(format!("{e} Reset {} to fix this.", catalog.specs[i].label));
        }
    }
}

/// Every shortcut's binding and the message for the ones that stay inactive. Input: the catalog and the store.
pub fn load_bindings(catalog: &Catalog, store: &dyn Store) -> (Vec<Binding>, Vec<Option<String>>) {
    let (mut bindings, mut errors) = read(catalog, store);
    check_set(catalog, &mut bindings, &mut errors);
    (bindings, errors)
}
