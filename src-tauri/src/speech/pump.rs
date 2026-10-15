//! Purpose: The speech thread's message loop: what other threads send the supervisor, and the loop that blocks on them with the supervisor's deadline.
//! Contents: Msg — the messages; run — the loop (blocks until a message or the deadline: no polling, nothing runs while Cue is idle).

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Instant;

use super::protocol::Reply;
use super::supervisor::Supervisor;
use super::take::Done;

pub enum Msg {
    /// A dictation starts: get the worker loading.
    Begin(u64),
    /// 16 kHz audio for dictation `id`.
    Audio(u64, Vec<f32>),
    /// The user stopped; `done` gets the whole text.
    End(u64, Done),
    Cancel,
    /// Kill the worker and answer once it is gone (the caller is about to delete its files).
    Unload(Sender<()>),
    Reply(u64, Reply),
    Gone(u64),
}

pub fn run(mut sup: Supervisor, rx: Receiver<Msg>) {
    loop {
        // Blocks until a message, or until the load / run / idle deadline. No polling.
        let msg = match sup.deadline {
            Some((at, why)) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Err(RecvTimeoutError::Timeout) => {
                    sup.expired(why);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(m) => m,
            },
            None => match rx.recv() {
                Ok(m) => m,
                Err(_) => return,
            },
        };
        match msg {
            Msg::Begin(id) => sup.begin(id),
            Msg::Audio(id, samples) => sup.audio(id, samples),
            Msg::End(id, done) => sup.end(id, done),
            Msg::Cancel => sup.cancel(),
            Msg::Unload(ack) => {
                sup.unload();
                let _ = ack.send(());
            }
            Msg::Reply(serial, reply) => sup.reply(serial, reply),
            Msg::Gone(serial) => sup.gone(serial),
        }
    }
}
