//! File: lock.rs
//! Purpose: The one way Cue takes a mutex.
//! Contents: lock — take a mutex and use it even when a panicking thread poisoned it.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Takes a mutex. A poisoned lock is still used, because every value guarded here is replaced whole or re-validated and a panic is already logged by `guard`. Input: the mutex. Returns the guard.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::lock;
    use std::sync::{Arc, Mutex};

    /// A lock poisoned by a panicking holder still yields its value and a write through it sticks.
    #[test]
    fn poisoned_lock_is_still_used() {
        let m = Arc::new(Mutex::new(1));
        let m2 = Arc::clone(&m);
        let _ = std::thread::spawn(move || {
            let _held = m2.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert!(m.is_poisoned());
        assert_eq!(*lock(&m), 1);
        *lock(&m) = 2;
        assert_eq!(*lock(&m), 2);
    }
}
