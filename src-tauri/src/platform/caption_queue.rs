//! File: platform/caption_queue.rs
//! Purpose: The queue of live captions waiting for the overlay thread, each stamped with the recording that said it, so a caption of an earlier recording is dropped when taken instead of showing under the next one.
//! Contents: Queue — new / start / push / withdraw / take; the owner lives in the queue so it changes and drains under one lock.

/// Captions as (ticket, recording id, sentence number, text), in the order they were pushed; the ticket names one push so it can be withdrawn.
pub(crate) struct Queue {
    owner: u64,
    next: u64,
    items: Vec<(u64, u64, usize, String)>,
}

impl Queue {
    /// An empty queue, usable in a `static`.
    pub(crate) const fn new() -> Self {
        Self { owner: 0, next: 0, items: Vec::new() }
    }

    /// Recording `id` takes the pill: drops every queued caption and makes `id` the owner.
    pub(crate) fn start(&mut self, id: u64) {
        self.owner = id;
        self.items.clear();
    }

    /// Queues sentence `index` with `text`, spoken by recording `id`. Returns the ticket that `withdraw` takes.
    pub(crate) fn push(&mut self, id: u64, index: usize, text: &str) -> u64 {
        self.next += 1;
        self.items.push((self.next, id, index, text.to_string()));
        self.next
    }

    /// Drops the entry pushed under `ticket`, for a post that found no overlay to hear it; entries pushed by others in between stay, and an entry already taken or cleared is not looked for again.
    pub(crate) fn withdraw(&mut self, ticket: u64) {
        self.items.retain(|(t, ..)| *t != ticket);
    }

    /// Drains the queue. Return: (sentence number, text) of the entries whose id is the owner, in order; the rest are dropped.
    pub(crate) fn take(&mut self) -> Vec<(usize, String)> {
        let owner = self.owner;
        std::mem::take(&mut self.items).into_iter().filter(|(_, id, ..)| *id == owner).map(|(_, _, i, t)| (i, t)).collect()
    }
}
