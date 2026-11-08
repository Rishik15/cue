//! File: platform/tests/mod.rs
//! Purpose: Tests of the platform-neutral logic that needs no operating system: how the popup card wraps its text and where it sits, and what the user is told when a delivery fails.
//! Contents: layout — flow, place and fit_logical; delivery — InsertError and InsertOptions.

mod delivery;
mod layout;
