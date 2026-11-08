//! File: rules/tests/mod.rs
//! Purpose: Tests of saved actions (rules): how they chain on dictated or selected text, which input they refuse, the Rules page dry run and the on-disk file.
//! Contents: chain — running rules in order; limits — what compile refuses; dry_run — the test panel; storage — rules.json format and recovery, run history; helpers — rule builders.

mod chain;
mod dry_run;
mod helpers;
mod limits;
mod storage;
