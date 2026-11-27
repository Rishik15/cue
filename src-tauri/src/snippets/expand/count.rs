//! File: snippets/expand/count.rs
//! Purpose: Keeps a snippet's use-count write ordered before any later `{counter}` read: the write is claimed before the expansion ends (before any key can start another one) and done after the late keys were handled, so the typing never waits for the database and no counter repeats a number. A write that fails stays owed: every later write and counter read writes it first, and a counter read is refused while it cannot be. An index swap reads without the gate (so an expansion's end never waits for that read) and holds the gate only for the swap, reading again when a use was counted meanwhile, so a use is never lost between them.
//! Contents: Pending — a claimed write (also the gate an index swap holds); claim — claim one; pay — write the owed uses; read — run a counter read after every claimed and owed write; swap — read outside the gate and hand back the result with the gate held.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::lock::lock;

/// The uses whose write failed and is still owed, oldest first; held from the claim of a use-count write until it is done, and `{counter}` reads wait on it.
static OWED: Mutex<Vec<i64>> = Mutex::new(Vec::new());

/// How many uses were written so far; raised under the gate after each one, so `swap` can tell that a use landed during its read.
static COUNTED: AtomicU64 = AtomicU64::new(0);

/// A use-count write that is claimed and not yet done; reads wait until it is dropped.
pub struct Pending {
    owed: MutexGuard<'static, Vec<i64>>,
}

impl Pending {
    /// Writes one use of snippet `id` through `bump`, after the uses still owed, then lets waiting reads go. Inputs: the snippet and the write. Returns the first failure; that use and the ones after it stay owed.
    pub fn write(mut self, id: i64, bump: impl FnMut(i64) -> Result<(), String>) -> Result<(), String> {
        self.owed.push(id);
        pay(&mut self.owed, bump)
    }
}

/// Claims the next use-count write; call it before the expansion ends. Returns the claim.
pub fn claim() -> Pending {
    Pending { owed: lock(&OWED) }
}

/// Writes the owed uses through `bump`, oldest first. Inputs: the owed list and the write. Returns the first failure; it and the later ones stay owed.
fn pay(owed: &mut Vec<i64>, mut bump: impl FnMut(i64) -> Result<(), String>) -> Result<(), String> {
    while let Some(&id) = owed.first() {
        bump(id)?;
        COUNTED.fetch_add(1, Ordering::SeqCst);
        owed.remove(0);
    }
    Ok(())
}

/// Runs a `{counter}` read once every write claimed before it is done and every owed use is written through `bump`. Inputs: the write and the read. Returns the read's result, or the refusal when an owed use cannot be written (the count would be behind, so its number could repeat).
pub fn read<T>(bump: impl FnMut(i64) -> Result<(), String>, read: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let mut owed = lock(&OWED);
    pay(&mut owed, bump).map_err(|e| {
        format!("A snippet's use could not be saved, so a snippet with {{counter}} was not typed (its number could repeat): {e}")
    })?;
    read()
}

/// Runs `read` without the gate, then claims the gate; when a use was written during the read (it may be missing from what the read saw and from the index it replaces), reads again. Each repeat needs a new use written during the read, so it ends as soon as one read passes without one. Input: the read. Returns what the read gave with the gate held (keep it until the result is in place, so no use lands between), or the read's failure.
pub fn swap<T, E>(mut read: impl FnMut() -> Result<T, E>) -> Result<(T, Pending), E> {
    loop {
        let seen = COUNTED.load(Ordering::SeqCst);
        let fresh = read()?;
        let gate = claim();
        if COUNTED.load(Ordering::SeqCst) == seen {
            return Ok((fresh, gate));
        }
    }
}
