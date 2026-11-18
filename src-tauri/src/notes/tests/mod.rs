//! File: notes/tests/mod.rs
//! Purpose: Tests of notes as a user relies on them: the math written on any line, and the note library (writing, moving between notes, trash, pins, search, expiry) through the page's one command.
//! Contents: math — results beside lines; library — the commands over a real SQLite file; opening — the shortcut after a failed opening.

mod library;
mod math;
mod opening;
