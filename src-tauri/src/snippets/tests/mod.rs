//! File: snippets/tests/mod.rs
//! Purpose: Tests of snippets as a user relies on them: saving and clashing triggers, upgrading an old database, importing from other tools, expansion while typing and the placeholders in the text.
//! Contents: helpers — builders; library — the database and folders; migration — old databases; flow — what a key starts, how an expansion ends and what stop or a new index does to pending work; imports — Cue files and bulk import; foreign — other tools' files; typing — trigger detection; templates — placeholders; limits — checks, the trigger set and program matching; patterns — the bound on `re:` triggers; keys — the page's settings; policy — saved snippets and the page's rules against the real matcher.

mod flow;
mod foreign;
mod helpers;
mod imports;
mod keys;
mod library;
mod limits;
mod migration;
mod patterns;
mod policy;
mod templates;
mod typing;
