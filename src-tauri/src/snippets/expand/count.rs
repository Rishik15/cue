//! File: snippets/expand/count.rs
//! Purpose: Keeps a snippet's use-count write ordered before any later `{counter}` read: the write is claimed before the expansion ends (before any key can start another one) and done after the late keys were handled, so the typing never waits for the database and no counter repeats a number.
//! Contents: Pending — a claimed write; claim — claim one; read — run a counter read after any claimed write.

use std::sync::{Mutex, MutexGuard};

use crate::lock::lock;

/// Held from the claim of a use-count write until it is done; `{counter}` reads wait on it.
static WRITING: Mutex<()> = Mutex::new(());

/// A use-count write that is claimed and not yet done; reads wait until it is dropped.
pub struct Pending {
    _held: MutexGuard<'static, ()>,
}

impl Pending {
    /// Does the write, then lets waiting reads go. Input: the write.
    pub fn write(self, write: impl FnOnce()) {
        write();
    }
}

/// Claims the next use-count write; call it before the expansion ends. Returns the claim.
pub fn claim() -> Pending {
    Pending { _held: lock(&WRITING) }
}

/// Runs a `{counter}` read once every write claimed before it is done. Input: the read. Returns its result.
pub fn read<T>(read: impl FnOnce() -> T) -> T {
    let _after_writes = lock(&WRITING);
    read()
}
