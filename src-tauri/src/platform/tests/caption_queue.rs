//! File: platform/tests/caption_queue.rs
//! Purpose: Protect the rule that a caption belongs to the recording that spoke it: the pill never draws words of an earlier recording.
//! Contents: tests of `Queue`.

use crate::platform::caption_queue::Queue;

/// A caption queued after a new owner took the pill (the stop-then-press race) is dropped.
#[test]
fn an_old_caption_is_dropped_after_a_new_owner() {
    let mut queue = Queue::new();
    queue.start(2);
    queue.push(1, 0, "old");
    assert!(queue.take().is_empty());
}

/// Only the owner's captions come out, in order, and the queue is empty afterwards.
#[test]
fn current_captions_come_out_in_order() {
    let mut queue = Queue::new();
    queue.start(2);
    queue.push(2, 0, "a");
    queue.push(2, 1, "b");
    queue.push(1, 0, "x");
    assert_eq!(queue.take(), vec![(0, "a".to_string()), (1, "b".to_string())]);
    assert!(queue.take().is_empty());
}

/// `withdraw` removes only its own push: a second push between a push and its failed post keeps its entry.
#[test]
fn withdraw_removes_only_its_own_entry() {
    let mut queue = Queue::new();
    queue.start(1);
    let first = queue.push(1, 0, "a");
    queue.push(1, 1, "b");
    queue.withdraw(first);
    assert_eq!(queue.take(), vec![(1, "b".to_string())]);
}

/// A new owner's captions survive a drain that began for the old owner: the owner change and the drain are one step of the queue, so the new words are never judged by the old owner.
#[test]
fn a_new_owners_captions_are_kept_when_the_owner_changes() {
    let mut queue = Queue::new();
    queue.start(1);
    queue.push(1, 0, "old");
    queue.start(2);
    queue.push(2, 0, "new");
    assert_eq!(queue.take(), vec![(0, "new".to_string())]);
}
