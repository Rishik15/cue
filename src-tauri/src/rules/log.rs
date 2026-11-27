//! File: rules/log.rs
//! Purpose: A short in-memory history of which rules matched, for the Rules page ("why did this change my text?"). Never holds the text itself,
//! and is gone when Cue quits, so it costs nothing on disk and nothing while idle.
//! Contents: Entry — one run; record / recent — add and read, newest first, at most `KEEP`.

use crate::lock::lock;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::model::Trigger;

const KEEP: usize = 30;

static RUNS: Mutex<VecDeque<Entry>> = Mutex::new(VecDeque::new());

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Entry {
    /// Seconds since 1970.
    pub at: u64,
    pub trigger: Trigger,
    pub matched: Vec<String>,
    pub app: String,
}

/// Notes one run that matched: when, its trigger, the rules that matched and the program; the oldest entry goes past `KEEP`. Inputs: the trigger, the rule names and the program.
pub fn record(trigger: Trigger, matched: &[String], app: String) {
    let at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut runs = lock(&RUNS);
    if runs.len() == KEEP {
        runs.pop_back();
    }
    runs.push_front(Entry { at, trigger, matched: matched.to_vec(), app });
}

/// The runs noted, newest first. Returns the entries.
pub fn recent() -> Vec<Entry> {
    lock(&RUNS).iter().cloned().collect()
}
