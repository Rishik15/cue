//! File: snippets/tests/timing.rs
//! Purpose: Measurements of the snippet library's write path, run by hand (ignored by default; timings are not judged on shared machines).
//! Contents: save_time_on_a_full_library — prints save p50 and p95 on a library at the snippet limit with the longest texts; toggle_wait_behind_an_import — prints how long a match case change waits behind an import.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use rusqlite::params;
use serde_json::json;

use super::helpers::{draft, open_db, save};
use crate::snippets::bulk::add_all;
use crate::snippets::check::{MAX_BODY, MAX_SNIPPETS};
use crate::snippets::policy::Policy;
use crate::snippets::{checked_setting, keys, write_db};

/// Saves this many snippets on top of the seeded library and times each.
const SAVES: usize = 50;

/// Snippets in the timed import.
const IMPORT: usize = 500;

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

/// Starts an import of `IMPORT` snippets (texts of `MAX_BODY` characters) under the one write lock, then a match case change: prints the import's time and how long after its end the change returned.
/// Run: `cargo test --release --lib toggle_wait_behind_an_import -- --ignored --nocapture`.
#[test]
#[ignore = "measurement, run by hand"]
fn toggle_wait_behind_an_import() {
    let (dir, _conn) = open_db();
    let db = dir.path().join("snippets.db");
    let body = "x".repeat(MAX_BODY);
    let entries: Vec<_> = (0..IMPORT).map(|i| (draft(&format!("I{i}"), &[&format!("i{i}")], &body), String::new())).collect();
    let (began, importing) = mpsc::channel();
    let importer = {
        let db = db.clone();
        std::thread::spawn(move || {
            write_db(&db, |c| {
                let start = Instant::now();
                began.send(()).expect("began");
                add_all(c, &entries, &Policy::default())?;
                Ok((start, Instant::now()))
            })
        })
    };
    importing.recv().expect("the import began");
    checked_setting(&db, Policy::default, keys::MATCH_CASE, &json!(true), || Ok(())).expect("the change");
    let returned = Instant::now();
    let (start, end) = importer.join().expect("importer").expect("import");
    println!(
        "import of {IMPORT}: {:?}; the change returned {:?} after the import ended",
        end - start,
        returned.saturating_duration_since(end)
    );
}
