//! File: model_download/tests/mod.rs
//! Purpose: Tests of model downloads as a user relies on them: several run side by side, a file only gets its real name when its hash matched, resume and cancel work, and a hostile server cannot make Cue write something it did not verify.
//! Contents: server — a loopback HTTP server; manager — the table of downloads; transfer — fetching one file; sizes — disk room.

mod manager;
mod server;
mod sizes;
mod transfer;
