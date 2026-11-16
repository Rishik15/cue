//! File: dictation/tests/mod.rs
//! Purpose: Tests of the dictation text path: cleaning a transcript with the user's vocabulary, fitting it to the caret, and what a hotkey press or release means in each mode.
//! Contents: abandon — cleanup only for an owned ending; transcript — fillers and vocabulary; caret — context-aware fit; hotkey — the mode table; route — where a result goes.

mod abandon;
mod caret;
mod hotkey;
mod route;
mod transcript;
