//! File: model_download/interrupt.rs
//! Purpose: Make blocking network calls end the moment a download is cancelled, on every OS: the call runs on a helper thread and the download thread waits on a channel that the cancel also writes to. The abandoned helper ends by its own deadline (connect, resolve, idle read).
//! Contents: interruptible — run one blocking call; PumpedBody — a body read through a helper thread.

use std::io::{self, Cursor, Read};
use std::sync::mpsc::{channel, sync_channel, Receiver, SyncSender};

use super::state::Ctl;
use super::transfer::Stop;

/// Bytes the helper thread reads at a time.
const CHUNK: usize = 64 * 1024;

/// Runs `work` on a helper thread and returns its result; returns `Stop::Cancelled` at once when `ctl` is cancelled first (the helper is left to end by its own deadline), `Stop::Failed` when the thread cannot start.
pub(super) fn interruptible<T: Send + 'static>(ctl: &Ctl, work: impl FnOnce() -> T + Send + 'static) -> Result<T, Stop> {
    let (done, answer) = channel::<Option<T>>();
    let wake = done.clone();
    let id = ctl.register(Box::new(move || {
        let _ = wake.send(None);
    }));
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

/// What the helper thread sends: a chunk (empty at the end of the body), or the cancel.
enum Piece {
    Chunk(io::Result<Vec<u8>>),
    Cancelled,
}

/// A body read on a helper thread, so a read blocked on a silent connection ends at once when the download is cancelled.
pub(super) struct PumpedBody {
    pieces: Receiver<Piece>,
    pending: Cursor<Vec<u8>>,
}

impl PumpedBody {
    /// Starts reading `source` on a helper thread; registers the cancel with `ctl`. When the thread cannot start the read fails with that error.
    pub(super) fn new(mut source: impl Read + Send + 'static, ctl: &Ctl) -> PumpedBody {
        let (send, pieces) = sync_channel::<Piece>(2);
        let cancel: SyncSender<Piece> = send.clone();
        ctl.register(Box::new(move || {
            let _ = cancel.try_send(Piece::Cancelled);
        }));
        let failed = send.clone();
        let spawned = crate::guard::spawn("download-body", 256 * 1024, move || loop {
            let mut buf = vec![0u8; CHUNK];
            let read = source.read(&mut buf).map(|n| {
                buf.truncate(n);
                buf
            });
            let last = !matches!(&read, Ok(chunk) if !chunk.is_empty());
            if send.send(Piece::Chunk(read)).is_err() || last {
                return;
            }
        });
        if let Err(e) = spawned {
            let _ = failed.try_send(Piece::Chunk(Err(e)));
        }
        PumpedBody { pieces, pending: Cursor::new(Vec::new()) }
    }
}

impl Read for PumpedBody {
    /// Serves the current chunk, then waits for the next one; a cancel is an `Interrupted` error.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let n = self.pending.read(buf)?;
            if n > 0 || buf.is_empty() {
                return Ok(n);
            }
            match self.pieces.recv() {
                Ok(Piece::Chunk(Ok(chunk))) if chunk.is_empty() => return Ok(0),
                Ok(Piece::Chunk(Ok(chunk))) => self.pending = Cursor::new(chunk),
                Ok(Piece::Chunk(Err(e))) => return Err(e),
                Ok(Piece::Cancelled) | Err(_) => return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled")),
            }
        }
    }
}
