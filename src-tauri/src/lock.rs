//! File: lock.rs
//! Purpose: The one way Cue takes a mutex or a read-write lock, or waits on a condition variable.
//! Contents: lock, read_lock, write_lock — take the lock and use it even when a panicking thread poisoned it; wait, wait_timeout, wait_timeout_while — the same policy for condition-variable waits.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

/// Takes a mutex. A poisoned lock is still used, because every value guarded here is replaced whole or re-validated and a panic is already logged by `guard`. Input: the mutex. Returns the guard.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Takes a read-write lock for reading, under the same poison policy as `lock`. Input: the lock. Returns the read guard.
pub fn read_lock<T>(m: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    m.read().unwrap_or_else(PoisonError::into_inner)
}

/// Takes a read-write lock for writing, under the same poison policy as `lock`. Input: the lock. Returns the write guard.
pub fn write_lock<T>(m: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    m.write().unwrap_or_else(PoisonError::into_inner)
}

/// Waits on a condition variable, under the same poison policy as `lock`. Inputs: the condition variable and the held guard. Returns the guard, held again.
pub fn wait<'a, T>(cond: &Condvar, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
    cond.wait(guard).unwrap_or_else(PoisonError::into_inner)
}

/// Waits on a condition variable for at most `limit`, under the same poison policy as `lock`. Inputs: the condition variable, the held guard and the limit. Returns the guard, held again; the caller re-checks its own deadline.
pub fn wait_timeout<'a, T>(cond: &Condvar, guard: MutexGuard<'a, T>, limit: Duration) -> MutexGuard<'a, T> {
    cond.wait_timeout(guard, limit).unwrap_or_else(PoisonError::into_inner).0
}

/// Waits on a condition variable while `keep_waiting` holds, for at most `limit`, under the same poison policy as `lock`. Inputs: the condition variable, the held guard, the limit and the predicate. Returns the guard, held again.
pub fn wait_timeout_while<'a, T>(
    cond: &Condvar,
    guard: MutexGuard<'a, T>,
    limit: Duration,
    keep_waiting: impl FnMut(&mut T) -> bool,
) -> MutexGuard<'a, T> {
    cond.wait_timeout_while(guard, limit, keep_waiting).unwrap_or_else(PoisonError::into_inner).0
}

#[cfg(test)]
mod tests {
    use super::{lock, read_lock, write_lock};
    use std::sync::{Arc, Mutex, RwLock};

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

    /// A read-write lock poisoned by a panicking writer still reads and writes.
    #[test]
    fn poisoned_rwlock_is_still_used() {
        let m = Arc::new(RwLock::new(1));
        let m2 = Arc::clone(&m);
        let _ = std::thread::spawn(move || {
            let _held = m2.write().unwrap();
            panic!("poison");
        })
        .join();
        assert!(m.is_poisoned());
        *write_lock(&m) = 2;
        assert_eq!(*read_lock(&m), 2);
    }
}
