//! File: snippets/tests/mod.rs
//! Purpose: Tests of snippets as a user relies on them: saving and clashing triggers, upgrading an old database, importing from other tools, expansion while typing and the placeholders in the text.
//! Contents: helpers — builders; library — the database and folders; migration — old databases; phase — expansion stages; flow — what a key starts and how an expansion ends; imports — Cue files and bulk import; foreign — other tools' files; typing — trigger detection; templates — placeholders; limits — checks and program matching.

mod flow;
mod foreign;
mod helpers;
mod imports;
mod library;
mod limits;
mod migration;
mod phase;
mod templates;
mod typing;
