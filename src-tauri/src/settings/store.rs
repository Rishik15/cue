//! File: settings/store.rs
//! Purpose: The settings held in memory: one snapshot readers share without waiting for the disk, and one serialized way to change it that reaches the disk before anyone sees the change.
//! Contents: Store — the file path, the current snapshot and the writer lock; open — read the file; snapshot / refusal / is_newer — reads; update — change keys, save atomically, then publish.
//! A change is applied to a copy, saved, and only then swapped in, so a failed save needs no rollback and a refused or failed change is never visible to readers or to the next save.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use serde_json::Value;

use super::file::{self, Loaded, Map, Mode, NEWER, VERSION, VERSION_KEY};

/// What readers see: the mode and the map, replaced whole on every successful change.
struct Current {
    mode: Mode,
    map: Arc<Map>,
}

/// The settings file's path, its current contents and the lock that serializes changes.
pub struct Store {
    path: PathBuf,
    current: RwLock<Current>,
    writers: Mutex<()>,
}

impl Store {
    /// Reads the settings file. Input: its path. Returns the store and what to tell the user about the file, if anything.
    pub fn open(path: PathBuf) -> (Self, Option<String>) {
        let Loaded { mode, map, notice } = file::load(&path);
        (Store { path, current: RwLock::new(Current { mode, map: Arc::new(map) }), writers: Mutex::new(()) }, notice)
    }

    /// The settings as they are now. Returns a shared snapshot; it never changes after it is returned.
    pub fn snapshot(&self) -> Arc<Map> {
        Arc::clone(&self.current.read().unwrap_or_else(PoisonError::into_inner).map)
    }

    /// Why changes are refused. Returns None when the settings may be changed.
    pub fn refusal(&self) -> Option<String> {
        match &self.current.read().unwrap_or_else(PoisonError::into_inner).mode {
            Mode::Ready => None,
            Mode::Newer => Some(NEWER.into()),
            Mode::Blocked(message) => Some(message.clone()),
        }
    }

    /// Whether the file was written by a newer Cue. Returns the verdict.
    pub fn is_newer(&self) -> bool {
        self.current.read().unwrap_or_else(PoisonError::into_inner).mode == Mode::Newer
    }

    /// Applies changes to a copy of the settings. Inputs: the current map and the changes (a key with Some sets it, with None removes it). Returns the new map, or None when nothing would change.
    fn applied(current: &Map, changes: &[(&str, Option<Value>)]) -> Option<Map> {
        let mut next = current.clone();
        for (key, value) in changes {
            match value {
                Some(v) => next.insert((*key).to_string(), v.clone()),
                None => next.remove(*key),
            };
        }
        next.insert(VERSION_KEY.into(), VERSION.into());
        (next != *current).then_some(next)
    }

    /// Changes several keys as one save. Input: the changes. Returns Err with a user-readable text when the file is newer, blocked or cannot be written; nothing is changed then, in memory or on disk. A change that alters nothing writes nothing.
    pub fn update(&self, changes: &[(&str, Option<Value>)]) -> Result<(), String> {
        let _writing = self.writers.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(why) = self.refusal() {
            return Err(why);
        }
        let Some(next) = Self::applied(&self.snapshot(), changes) else { return Ok(()) };
        file::write_atomic(&self.path, &next).map_err(|e| format!("Could not save the settings: {e}"))?;
        self.current.write().unwrap_or_else(PoisonError::into_inner).map = Arc::new(next);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
