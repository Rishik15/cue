//! File: text/tests/mod.rs
//! Purpose: Tests of the text model's safety net: how text is cut into units, which model replies are trusted, and that a rejected reply leaves the user's own words in place.
//! Contents: pipeline — split, check and join as one edit; replies — what `accept` lets through and refuses.

mod pipeline;
mod replies;
