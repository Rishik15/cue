//! File: platform/tests/mod.rs
//! Purpose: Tests of the platform-neutral logic that needs no operating system: what the user is told when a delivery fails.
//! Contents: cleanup_job — the remove-data command line and folder deletion; layout — fit_logical; delivery — InsertError and InsertOptions; watcher_state — the watcher lifecycle; typing_policy — typed burst or paste.

mod cleanup_job;
mod delivery;
mod layout;
mod typing_policy;
mod watcher_state;
