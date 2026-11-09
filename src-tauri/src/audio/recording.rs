//! File: audio/recording.rs
//! Purpose: The one owner of a recording in progress: a fixed-capacity ring the realtime callback writes and the audio thread drains. The callback
//! only stores samples into the ring and commits with one atomic compare-and-swap on a word that holds the recording flag, the live flag, the
//! overflow flag, the recording's generation and the count written; it takes no lock and never allocates. A callback that was already running
//! when the recording stopped (or a new one began) fails the swap and its samples are discarded, so a stop never loses the tail it counted, a
//! late callback never adds to the next recording and never turns the live flag back on, and memory is fixed.
//! Contents: Recording — new, start, write (the callback), drain, stop, full, live, level; Drain — what the audio thread takes.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

const RECORDING: u64 = 1 << 63;
const LIVE: u64 = 1 << 62;
const OVERFLOW: u64 = 1 << 61;
const GENERATION: u64 = 0x1F_FFFF << COUNT_BITS;
const COUNT_BITS: u32 = 40;
const COUNT: u64 = (1 << COUNT_BITS) - 1;

/// The ring and the atomic words one recording lives in; see the module header.
pub struct Recording {
    ring: Box<[AtomicU32]>,
    cursor: AtomicU64,
    tail: AtomicU64,
    max: AtomicUsize,
    level: AtomicU32,
}

/// What the audio thread took from the ring.
pub struct Drain {
    pub samples: Vec<f32>,
    pub overflowed: bool,
}

/// A callback's view of the recording when it began, so its commit can tell whether the recording changed meanwhile.
pub struct Claim {
    seen: u64,
    room: usize,
}

/// What a callback stored before it commits.
pub struct Fill {
    accepted: usize,
    frames: usize,
    ring_full: bool,
    sum: f32,
}

impl Recording {
    /// A ring of `capacity` mono samples, not recording. Input: the capacity (about five seconds at the device rate).
    pub fn new(capacity: usize) -> Arc<Self> {
        let ring = (0..capacity.max(1)).map(|_| AtomicU32::new(0)).collect();
        Arc::new(Recording { ring, cursor: AtomicU64::new(0), tail: AtomicU64::new(0), max: AtomicUsize::new(0), level: AtomicU32::new(0) })
    }

    /// Begins a new recording, discarding anything left. Input: the most samples it may hold in total (the user's recording limit).
    pub fn start(&self, max_samples: usize) {
        let generation = ((self.cursor.load(Ordering::Acquire) & GENERATION) + (1 << COUNT_BITS)) & GENERATION;
        self.tail.store(0, Ordering::Release);
        self.max.store(max_samples, Ordering::Release);
        self.level.store(0, Ordering::Relaxed);
        self.cursor.store(RECORDING | generation, Ordering::Release);
    }

    /// The realtime callback: mixes `data` down to mono and stores it. Inputs: interleaved samples, the channel count and the conversion to f32.
    pub fn write<T>(&self, data: &[T], channels: usize, to_f32: impl Fn(&T) -> f32) {
        let Some(claim) = self.claim() else { return };
        let fill = self.fill(&claim, data, channels, to_f32);
        self.commit(&claim, &fill);
    }

    /// Reads the recording state at the start of a callback. Returns None when nothing is recording.
    pub fn claim(&self) -> Option<Claim> {
        let seen = self.cursor.load(Ordering::Acquire);
        if seen & RECORDING == 0 {
            return None;
        }
        let used = (seen & COUNT).saturating_sub(self.tail.load(Ordering::Acquire)) as usize;
        let ring_room = self.ring.len().saturating_sub(used);
        let cap_room = self.max.load(Ordering::Acquire).saturating_sub((seen & COUNT) as usize);
        Some(Claim { seen, room: ring_room.min(cap_room) })
    }

    /// Stores the mixed-down samples into the free slots of the ring without publishing them.
    pub fn fill<T>(&self, claim: &Claim, data: &[T], channels: usize, to_f32: impl Fn(&T) -> f32) -> Fill {
        let channels = channels.max(1);
        let start = (claim.seen & COUNT) as usize;
        let mut fill = Fill { accepted: 0, frames: 0, ring_full: false, sum: 0.0 };
        for frame in data.chunks_exact(channels) {
            let mono = frame.iter().map(&to_f32).sum::<f32>() / channels as f32;
            fill.sum += mono * mono;
            fill.frames += 1;
            if fill.accepted < claim.room {
                self.ring[(start + fill.accepted) % self.ring.len()].store(mono.to_bits(), Ordering::Relaxed);
                fill.accepted += 1;
            }
        }
        let cap_room = self.max.load(Ordering::Acquire).saturating_sub(start);
        fill.ring_full = fill.frames > fill.accepted && claim.room < cap_room;
        fill
    }

    /// Publishes a fill with one swap. Returns false, and publishes nothing, when the recording stopped or restarted since the claim.
    pub fn commit(&self, claim: &Claim, fill: &Fill) -> bool {
        let count = (claim.seen & COUNT) + fill.accepted as u64;
        let flags = LIVE | if fill.ring_full { OVERFLOW } else { 0 };
        let next = ((claim.seen | flags) & !COUNT) | count;
        if self.cursor.compare_exchange(claim.seen, next, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return false;
        }
        if fill.frames > 0 {
            self.level.store((fill.sum / fill.frames as f32).sqrt().to_bits(), Ordering::Relaxed);
        }
        true
    }

    /// Takes everything written since the last drain. Only the audio thread calls this.
    pub fn drain(&self) -> Drain {
        self.take(self.cursor.load(Ordering::Acquire))
    }

    /// Ends the recording and takes the rest. After this returns no callback can add to it, so the tail is complete.
    pub fn stop(&self) -> Drain {
        let mut seen = self.cursor.load(Ordering::Acquire);
        while let Err(now) = self.cursor.compare_exchange(seen, seen & !(RECORDING | LIVE), Ordering::AcqRel, Ordering::Acquire) {
            seen = now;
        }
        self.level.store(0, Ordering::Relaxed);
        self.take(seen)
    }

    /// Copies the ring from the last drain up to the count in `seen` and moves the tail there. Input: a cursor value. Returns the samples and the overflow flag.
    fn take(&self, seen: u64) -> Drain {
        let (from, to) = (self.tail.load(Ordering::Acquire), seen & COUNT);
        let samples = (from..to).map(|n| f32::from_bits(self.ring[n as usize % self.ring.len()].load(Ordering::Relaxed))).collect();
        self.tail.store(to, Ordering::Release);
        Drain { samples, overflowed: seen & OVERFLOW != 0 }
    }

    /// Whether the recording has reached its limit.
    pub fn full(&self) -> bool {
        (self.cursor.load(Ordering::Relaxed) & COUNT) as usize >= self.max.load(Ordering::Relaxed)
    }

    /// Whether the microphone has delivered audio to the recording in progress.
    pub fn live(&self) -> bool {
        self.cursor.load(Ordering::Relaxed) & (RECORDING | LIVE) == RECORDING | LIVE
    }

    /// RMS of the latest callback, 0 to 1.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}
