//! File: settings/file.rs
//! Purpose: The settings file on disk: reading it into a state the rest of Cue can trust, and replacing it without ever leaving a half-written file.
//! Contents: Map — the flat key map; migrate — upgrade an older layout in memory; Mode — whether the map may be changed; Loaded — a read file; load — read, classify and, when damaged, set aside; write_atomic — write a temporary file in the same folder, sync it and rename it over the old one.
//! A file Cue cannot read or move is never touched: it stays as evidence and writes are refused, so settings are never replaced by defaults over a transient fault.

use std::io::Write;
use std::path::Path;

use serde_json::Value;

/// The settings as one flat JSON object, in file order.
pub type Map = serde_json::Map<String, Value>;

/// One upgrade of the key map from one layout version to the next.
type Step = fn(&mut Map);

/// The upgrades, oldest first: step `i` takes layout `i + 1` to `i + 2`. Empty until a key changes meaning; add the step here and the version rises with it.
const MIGRATIONS: &[Step] = &[];

/// The layout version this build writes under `settings_version`. A file stamped higher belongs to a newer Cue: it is read but never written.
pub const VERSION: u64 = 1 + MIGRATIONS.len() as u64;
/// The key holding the layout version.
pub const VERSION_KEY: &str = "settings_version";
/// The largest settings file Cue reads or writes, so a hostile or runaway file cannot multiply startup allocations.
pub const MAX_FILE_BYTES: u64 = 1 << 20;
/// What a damaged file is renamed to.
const DAMAGED: &str = "settings.damaged.json";
/// What the settings window is told when a write is refused because the file is from a newer Cue.
pub const NEWER: &str = "These settings were saved by a newer version of Cue. Update Cue to change them.";

/// Whether the settings may be changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Readable and current: reads and writes work.
    Ready,
    /// Written by a newer Cue: reads work, writes are refused and the file is never rewritten.
    Newer,
    /// Unreadable, too large or damaged beyond being set aside: reads give defaults, writes are refused with this text, the file is left alone.
    Blocked(String),
}

/// A settings file as read at startup.
pub struct Loaded {
    pub mode: Mode,
    pub map: Map,
    /// Something to tell the user once the tray exists; None when the file was fine or absent.
    pub notice: Option<String>,
}

/// An empty, writable result with an optional notice. Input: the notice. Returns it.
fn fresh(notice: Option<String>) -> Loaded {
    Loaded { mode: Mode::Ready, map: Map::new(), notice }
}

/// A result that refuses writes. Input: why, as the end of the sentence "Cue's settings file ...". Returns it with the message as both the mode text and the notice.
fn blocked(why: &str) -> Loaded {
    let message = format!("Cue's settings file {why}, so Cue left it alone and will not save settings. Close any program using it or delete it, then restart Cue.");
    Loaded { mode: Mode::Blocked(message.clone()), map: Map::new(), notice: Some(message) }
}

/// Reads the layout version a map carries. Input: the map. Returns 0 when absent or not a whole number.
fn version_of(map: &Map) -> u64 {
    map.get(VERSION_KEY).and_then(Value::as_u64).unwrap_or(0)
}

/// Brings a map from the layout it is stamped with to the current one, in memory; the new stamp reaches the disk with the next save. Inputs: the map and the ordered steps. Returns nothing.
fn migrate(map: &mut Map, steps: &[Step]) {
    let from = usize::try_from(version_of(map).max(1)).unwrap_or(usize::MAX);
    for step in steps.iter().skip(from - 1) {
        step(map);
    }
}

/// Sets a damaged file aside. Input: the file's path. Returns defaults with a notice when the file was moved, or a blocked result when it could not be, in which case it stays where it is.
fn set_aside(path: &Path) -> Loaded {
    match std::fs::rename(path, path.with_file_name(DAMAGED)) {
        Ok(()) => {
            fresh(Some("Your settings file was damaged, so Cue set it aside (settings.damaged.json) and started with the defaults.".into()))
        }
        Err(_) => blocked("is damaged and could not be set aside"),
    }
}

/// Reads the settings file. Input: its path. Returns the map and what may be done with it: a missing file is fresh defaults; a file that is not a JSON object is set aside; an unreadable or oversized file blocks writes; a newer one is read-only.
pub fn load(path: &Path) -> Loaded {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return fresh(None),
        Err(_) => return blocked("could not be read"),
        Ok(m) if m.len() > MAX_FILE_BYTES => return blocked("is larger than Cue reads"),
        Ok(_) => {}
    }
    let Ok(bytes) = std::fs::read(path) else { return blocked("could not be read") };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(map)) if version_of(&map) > VERSION => Loaded { mode: Mode::Newer, map, notice: None },
        Ok(Value::Object(mut map)) => {
            migrate(&mut map, MIGRATIONS);
            Loaded { mode: Mode::Ready, map, notice: None }
        }
        _ => set_aside(path),
    }
}

/// Writes and syncs a file. Inputs: path and bytes. Returns the I/O result.
fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Replaces the settings file with a map: the bytes go to a temporary file beside it, are synced, and the rename replaces the old file in one step, so a crash or a full disk leaves either the old file or the new one. Inputs: the path and the map. Returns an error text when the map is too large to read back or any step fails; the temporary file is removed then and the old file is untouched.
pub fn write_atomic(path: &Path, map: &Map) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(map).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("The settings are too large to save.".into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("json.tmp");
    let result = write_synced(&temp, &bytes).and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
pub(super) mod tests;
