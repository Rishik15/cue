//! File: platform/tests/caption_queue.rs
//! Purpose: Protect the rule that a caption belongs to the recording that spoke it: the pill never draws words of an earlier recording.
//! Contents: tests of `Queue`.

use crate::platform::caption_queue::Queue;

/// A caption queued after a new owner took the pill (the stop-then-press race) is dropped.
#[test]
fn an_old_caption_is_dropped_after_a_new_owner() {
    let mut queue = Queue::new();
    queue.push(1, 0, "old");
    assert!(queue.take_for(2).is_empty());
}

/// Only the owner's captions come out, in order, and the queue is empty afterwards.
#[test]
fn current_captions_come_out_in_order() {
    let mut queue = Queue::new();
    queue.push(2, 0, "a");
    queue.push(2, 1, "b");
    queue.push(1, 0, "x");
    assert_eq!(queue.take_for(2), vec![(0, "a".to_string()), (1, "b".to_string())]);
    assert!(queue.take_for(1).is_empty());
}

/// `pop` removes the newest entry only.
#[test]
fn pop_removes_the_last_entry() {
    let mut queue = Queue::new();
    queue.push(1, 0, "a");
    queue.push(1, 1, "b");
    queue.pop();
    assert_eq!(queue.take_for(1), vec![(0, "a".to_string())]);
}
