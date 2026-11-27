//! File: snippets/tests/timing.rs
//! Purpose: Measurements of the snippet library's write path, run by hand (ignored by default; timings are not judged on shared machines).
//! Contents: save_time_on_a_full_library — prints save p50 and p95 on a library at the snippet limit with the longest texts.

use std::time::{Duration, Instant};

use rusqlite::params;

use super::helpers::{draft, open_db, save};
use crate::snippets::check::{MAX_BODY, MAX_SNIPPETS};

/// Saves this many snippets on top of the seeded library and times each.
const SAVES: usize = 50;

/// Times `save` on a library seeded to the snippet limit with texts of `MAX_BODY` characters: prints p50 and p95 over `SAVES` saves.
/// Run: `cargo test --release --lib save_time_on_a_full_library -- --ignored --nocapture`.
#[test]
#[ignore = "measurement, run by hand"]
fn save_time_on_a_full_library() {
    let (_dir, conn) = open_db();
    let body = "x".repeat(MAX_BODY);
    let seeded = MAX_SNIPPETS - SAVES;
    conn.execute_batch("BEGIN").expect("begin");
    for i in 0..seeded {
        conn.execute("INSERT INTO snippets (name, triggers, body) VALUES (?1, ?2, ?3)", params![format!("S{i}"), format!("s{i}"), body])
            .expect("seed");
    }
    conn.execute_batch("COMMIT").expect("commit");
    let mut times: Vec<Duration> = (0..SAVES)
        .map(|i| {
            let start = Instant::now();
            save(&conn, None, &draft(&format!("N{i}"), &[&format!("n{i}")], "x")).expect("save");
            start.elapsed()
        })
        .collect();
    times.sort();
    println!("save on {seeded} snippets of {MAX_BODY} characters: p50 {:?}, p95 {:?}", times[SAVES / 2], times[SAVES * 95 / 100]);
}
