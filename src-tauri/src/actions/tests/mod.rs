//! File: actions/tests/mod.rs
//! Purpose: Tests of the text actions as the palette and saved actions run them: whole texts in, the text a user would see out.
//! Contents: catalog — the action list contract; links — link cleaning and extraction; cleanup — spacing, case, speech and number jobs; markdown — clipboard HTML to Markdown; properties — random-text invariants.

mod catalog;
mod cleanup;
mod links;
mod markdown;
mod properties;
