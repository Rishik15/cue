//! File: platform/caption_queue.rs
//! Purpose: The queue of live captions waiting for the overlay thread, each stamped with the recording that said it, so a caption of an earlier recording is dropped when taken instead of showing under the next one.
//! Contents: Queue — new / start / push / pop / take; the owner lives in the queue so it changes and drains under one lock.

/// Captions as (recording id, sentence number, text), in the order they were pushed.
pub(crate) struct Queue {
    owner: u64,
    items: Vec<(u64, usize, String)>,
}

impl Queue {
    /// An empty queue, usable in a `static`.
    pub(crate) const fn new() -> Self {
        Self { owner: 0, items: Vec::new() }
    }

    /// Recording `id` takes the pill: drops every queued caption and makes `id` the owner.
    pub(crate) fn start(&mut self, id: u64) {
        self.owner = id;
        self.items.clear();
    }

    /// Queues sentence `index` with `text`, spoken by recording `id`.
    pub(crate) fn push(&mut self, id: u64, index: usize, text: &str) {
        self.items.push((id, index, text.to_string()));
    }

    /// Drops the last entry, for a post that found no overlay to hear it.
    pub(crate) fn pop(&mut self) {
        self.items.pop();
    }

    /// Drains the queue. Return: (sentence number, text) of the entries whose id is the owner, in order; the rest are dropped.
    pub(crate) fn take(&mut self) -> Vec<(usize, String)> {
        let owner = self.owner;
        std::mem::take(&mut self.items).into_iter().filter(|(id, ..)| *id == owner).map(|(_, i, t)| (i, t)).collect()
    }
}
