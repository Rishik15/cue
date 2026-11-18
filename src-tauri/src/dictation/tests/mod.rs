//! File: dictation/tests/mod.rs
//! Purpose: Tests of the dictation text path: cleaning a transcript with the user's vocabulary, fitting it to the caret, what a hotkey press or release means in each mode, who owns a recording, and the timing line.
//! Contents: flow — ownership and completion; transcript — fillers and vocabulary; caret — context-aware fit; hotkey — the mode table; route — where a result goes; timing — the log line.

mod caret;
mod flow;
mod hotkey;
mod route;
mod timing;
mod transcript;
