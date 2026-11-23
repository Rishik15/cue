//! File: platform/caption_queue.rs
//! Purpose: The queue of live captions waiting for the overlay thread, each stamped with the recording that said it, so a caption of an earlier recording is dropped when taken instead of showing under the next one.
//! Contents: Queue — new / clear / push / pop / take_for.

/// Captions as (recording id, sentence number, text), in the order they were pushed.
pub(crate) struct Queue(Vec<(u64, usize, String)>);

impl Queue {
    /// An empty queue, usable in a `static`.
    pub(crate) const fn new() -> Self {
        Self(Vec::new())
    }

    /// Drops every queued caption.
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    /// Queues sentence `index` with `text`, spoken by recording `id`.
    pub(crate) fn push(&mut self, id: u64, index: usize, text: &str) {
        self.0.push((id, index, text.to_string()));
    }

    /// Drops the last entry, for a post that found no overlay to hear it.
    pub(crate) fn pop(&mut self) {
        self.0.pop();
    }

    /// Drains the queue. Input: `owner`, the recording the pill shows now. Return: (sentence number, text) of the entries whose id is `owner`, in order; the rest are dropped.
    pub(crate) fn take_for(&mut self, owner: u64) -> Vec<(usize, String)> {
        std::mem::take(&mut self.0).into_iter().filter(|(id, ..)| *id == owner).map(|(_, i, t)| (i, t)).collect()
    }
}
