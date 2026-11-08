//! File: input/mailbox.rs
//! Purpose: How requests reach the input thread: callers on any thread queue a command and wake the thread; the thread takes them in order. No thread is polled and nothing runs at idle.
//! Contents: Mailbox — the queue and the wake-up.

use std::collections::VecDeque;
use std::sync::Mutex;

/// A queue of `T` plus the function that wakes whoever drains it.
pub struct Mailbox<T> {
    queue: Mutex<VecDeque<T>>,
    wake: Box<dyn Fn() + Send + Sync>,
}

impl<T> Mailbox<T> {
    /// A mailbox that calls `wake` after every post.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Mailbox<T> {
        Mailbox { queue: Mutex::new(VecDeque::new()), wake: Box::new(wake) }
    }

    /// Queues `item` and wakes the input thread.
    pub fn post(&self, item: T) {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).push_back(item);
        (self.wake)();
    }

    /// Takes everything queued so far, in order.
    pub fn take(&self) -> VecDeque<T> {
        std::mem::take(&mut *self.queue.lock().unwrap_or_else(|e| e.into_inner()))
    }
}
