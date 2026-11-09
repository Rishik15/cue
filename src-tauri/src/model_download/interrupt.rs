//! File: model_download/interrupt.rs
//! Purpose: Make blocking network calls end the moment a download is cancelled, on every OS: the call runs on a helper thread and the download thread waits on a channel that the cancel also writes to. The abandoned helper ends by its own deadline (connect, resolve, idle read).
//! Contents: interruptible — run one blocking call; PumpedBody — a body read through a helper thread.

use std::io::{self, Read};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::Arc;

use super::ctl::Ctl;
use super::transfer::Stop;

/// Bytes the helper thread reads at a time.
const CHUNK: usize = 64 * 1024;

/// Runs `work` on a helper thread and returns its result; returns `Stop::Cancelled` at once when `ctl` is cancelled first (the helper is left to end by its own deadline) or already was (nothing is spawned), `Stop::Failed` when the thread cannot start. The interrupt is unregistered before returning.
pub(super) fn interruptible<T: Send + 'static>(ctl: &Ctl, work: impl FnOnce() -> T + Send + 'static) -> Result<T, Stop> {
    let (done, answer) = channel::<Option<T>>();
    let wake = done.clone();
    let id = ctl.register(Box::new(move || {
        let _ = wake.send(None);
    }));
    if id == 0 {
        return Err(Stop::Cancelled);
    }
    let spawned = crate::guard::spawn("download-net", 256 * 1024, move || {
        let _ = done.send(Some(work()));
    });
    let result = match spawned {
        Ok(()) => answer.recv().ok().flatten().ok_or(Stop::Cancelled),
        Err(e) => Err(Stop::Failed(format!("Could not start the download: {e}"))),
    };
    ctl.unregister(id);
    result
}

/// What the helper thread sends: a buffer and how much of it is data (0 at the end of the body), a read error, or the cancel.
enum Piece {
    Chunk(io::Result<(Vec<u8>, usize)>),
    Cancelled,
}

/// A body read on a helper thread, so a read blocked on a silent connection ends at once when the download is cancelled. Two buffers travel between the threads and are reused.
/// The helper thread (one per body, 256 KB stack, parked while the consumer is busy) is the cost of a sub-second cancel on Windows, where a blocked socket read cannot be woken; judge it by bench before widening its use.
pub(super) struct PumpedBody {
    pieces: Receiver<Piece>,
    spare: Sender<Vec<u8>>,
    pending: Vec<u8>,
    at: usize,
    len: usize,
    ctl: Option<Arc<Ctl>>,
    ids: Vec<u64>,
}

impl PumpedBody {
    /// Starts reading `source` on a helper thread and registers the cancel with `ctl`; `held` are ids of other interrupts (the socket closer) that live as long as this body and are unregistered when it is dropped. Nothing is spawned when `ctl` was cancelled already: the first read fails.
    pub(super) fn new(mut source: impl Read + Send + 'static, ctl: &Ctl, held: Vec<u64>) -> PumpedBody {
        let (send, pieces) = sync_channel::<Piece>(2);
        let (spare, spare_in) = channel::<Vec<u8>>();
        let cancel: SyncSender<Piece> = send.clone();
        let id = ctl.register(Box::new(move || {
            let _ = cancel.try_send(Piece::Cancelled);
        }));
        let failed = send.clone();
        let spawned = (id != 0).then(|| {
            crate::guard::spawn("download-body", 256 * 1024, move || loop {
                let mut buf = spare_in.try_recv().unwrap_or_else(|_| vec![0u8; CHUNK]);
                let read = source.read(&mut buf).map(|n| (buf, n));
                let last = !matches!(&read, Ok((_, n)) if *n > 0);
                if send.send(Piece::Chunk(read)).is_err() || last {
                    return;
                }
            })
        });
        if let Some(Err(e)) = spawned {
            let _ = failed.try_send(Piece::Chunk(Err(e)));
        }
        let ids = held.into_iter().chain(std::iter::once(id)).collect();
        PumpedBody { pieces, spare, pending: Vec::new(), at: 0, len: 0, ctl: ctl.shared(), ids }
    }
}

impl Drop for PumpedBody {
    /// Unregisters the interrupts of this body.
    fn drop(&mut self) {
        if let Some(ctl) = &self.ctl {
            self.ids.iter().for_each(|id| ctl.unregister(*id));
        }
    }
}

impl Read for PumpedBody {
    /// Serves the current buffer, then waits for the next one (handing the empty one back for reuse); a cancel is an `Interrupted` error.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.at < self.len {
                let n = buf.len().min(self.len - self.at);
                buf[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
                self.at += n;
                return Ok(n);
            }
            if buf.is_empty() {
                return Ok(0);
            }
            if !self.pending.is_empty() {
                let _ = self.spare.send(std::mem::take(&mut self.pending));
            }
            match self.pieces.recv() {
                Ok(Piece::Chunk(Ok((_, 0)))) => return Ok(0),
                Ok(Piece::Chunk(Ok((chunk, n)))) => (self.pending, self.at, self.len) = (chunk, 0, n),
                Ok(Piece::Chunk(Err(e))) => return Err(e),
                Ok(Piece::Cancelled) | Err(_) => return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled")),
            }
        }
    }
}
