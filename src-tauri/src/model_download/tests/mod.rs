//! File: model_download/tests/mod.rs
//! Purpose: Tests of model downloads as a user relies on them: several run side by side, a file only gets its real name when its hash matched, resume and cancel work, and a hostile server cannot make Cue write something it did not verify.
//! Contents: child — cancelling an unpacker; fakes — test adapters of the seams; worker — whole jobs against them; ledger — disk room by path; server — a loopback HTTP server; silent — cancelling a silent server; manager — the table of downloads; transfer — fetching one file; sizes — disk room.

mod child;
mod fakes;
mod ledger;
mod manager;
mod server;
mod silent;
mod sizes;
mod transfer;
mod worker;
