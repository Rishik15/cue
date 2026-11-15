//! File: dictation/tests/mod.rs
//! Purpose: Tests of the dictation text path: cleaning a transcript with the user's vocabulary, fitting it to the caret, and what a hotkey press or release means in each mode.
//! Contents: transcript — fillers and vocabulary; caret — context-aware fit; hotkey — the mode table; recording — begin and end of a recording.

mod caret;
mod hotkey;
mod recording;
mod transcript;
