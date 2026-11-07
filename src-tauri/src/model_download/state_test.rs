//! Purpose: Prove the download book-keeping: no cap, one download per model, stale generations, delete reservations, disk reservations, shared-file locks and shutdown.
//! Contents: one test per rule, each on its own `Manager`.

use std::time::Duration;

use super::*;

/// Sizes for a download of `n` bytes that has nothing on disk yet.
fn fresh(n: u64) -> Sizes {
    Sizes { total: n, need: n }
}

fn manager() -> &'static Manager {
    Box::leak(Box::new(Manager::new()))
}

#[test]
fn every_distinct_model_runs_at_once() {
    let m = manager();
    for id in ["a", "b", "c", "d", "e"] {
        m.admit(id, "speech", fresh(10), &|| u64::MAX).unwrap();
    }
    let view = m.view();
    assert!(["a", "b", "c", "d", "e"].iter().all(|id| view.job(id).is_some()));
}

#[test]
fn the_same_model_twice_is_refused_and_a_stale_generation_changes_nothing() {
    let m = manager();
    let first = m.admit("a", "speech", fresh(1), &|| u64::MAX).unwrap();
    assert!(m.admit("a", "speech", fresh(1), &|| u64::MAX).is_err());
    m.cancel("a", first.generation + 7);
    assert!(!first.ctl.is_cancelled());
    m.finish("a", first.generation);
    let second = m.admit("a", "speech", fresh(1), &|| u64::MAX).unwrap();
    assert!(second.generation > first.generation);
    m.finish("a", first.generation);
    assert!(m.view().job("a").is_some(), "a late finish of the old generation must not end the new one");
}

#[test]
fn a_download_that_does_not_fit_next_to_the_others_is_refused() {
    let m = manager();
    let first = m.admit("a", "speech", fresh(100), &|| 150).unwrap();
    assert!(m.admit("b", "speech", fresh(100), &|| 150).is_err(), "100 + 100 does not fit in 150");
    first.ctl.wrote(60);
    assert!(m.admit("b", "speech", fresh(50), &|| 90).is_ok(), "the first wrote 60: 40 owed + 50 fits in the 90 left");
    assert!(m.admit("c", "speech", fresh(10), &|| 90).is_err(), "40 + 50 owed leave nothing for 10 more in 90");
}

#[test]
fn bytes_thrown_away_are_reserved_until_written_again() {
    let m = manager();
    let job = m.admit("a", "speech", fresh(100), &|| 1000).unwrap();
    job.ctl.lose(900);
    assert!(m.admit("b", "speech", fresh(200), &|| 1000).is_err(), "1000 owed leave too little for 200");
    job.ctl.wrote(1000);
    assert!(m.admit("b", "speech", fresh(200), &|| 1000).is_ok(), "rewritten: nothing owed any more");
}

#[test]
fn delete_is_refused_while_downloading_and_a_start_is_refused_while_deleting() {
    let m = manager();
    m.admit("a", "speech", fresh(1), &|| u64::MAX).unwrap();
    assert!(m.reserve_delete("a", "speech").is_err());
    let held = m.reserve_delete("b", "speech").unwrap();
    assert!(m.admit("b", "speech", fresh(1), &|| u64::MAX).is_err());
    assert!(m.reserve_delete("b", "speech").is_err());
    drop(held);
    assert!(m.admit("b", "speech", fresh(1), &|| u64::MAX).is_ok());
}

#[test]
fn text_models_share_a_folder_so_a_text_delete_waits_for_text_downloads() {
    let m = manager();
    m.admit("q", "text", fresh(1), &|| u64::MAX).unwrap();
    assert!(m.reserve_delete("l", "text").is_err());
    assert!(m.view().delete_blocked("l", "text"));
    assert!(!m.view().delete_blocked("w", "speech"));
    assert!(m.reserve_delete("w", "speech").is_ok());
}

#[test]
fn a_second_user_of_a_shared_file_waits_and_a_cancel_wakes_it() {
    let m = manager();
    let (owner, waiter) =
        (m.admit("a", "speech", fresh(1), &|| u64::MAX).unwrap(), m.admit("b", "speech", fresh(1), &|| u64::MAX).unwrap());
    let held = m.lock("vad".into(), &owner.ctl).unwrap();
    let ctl = waiter.ctl.clone();
    let thread = std::thread::spawn(move || m.lock("vad".into(), &ctl).is_ok());
    std::thread::sleep(Duration::from_millis(200));
    assert!(!thread.is_finished(), "the second user must wait for the first");
    m.cancel("b", waiter.generation);
    assert!(!thread.join().unwrap(), "a cancelled waiter gives up");
    drop(held);
    assert!(m.lock("vad".into(), &owner.ctl).is_ok(), "the lock is free again");
}

#[test]
fn a_lock_goes_to_the_next_user_when_released() {
    let m = manager();
    let (a, b) = (m.admit("a", "speech", fresh(1), &|| u64::MAX).unwrap(), m.admit("b", "speech", fresh(1), &|| u64::MAX).unwrap());
    let held = m.lock("vad".into(), &a.ctl).unwrap();
    let ctl = b.ctl.clone();
    let thread = std::thread::spawn(move || m.lock("vad".into(), &ctl).is_ok());
    std::thread::sleep(Duration::from_millis(100));
    drop(held);
    assert!(thread.join().unwrap());
}

#[test]
fn shutdown_cancels_everything_and_waits_for_the_threads_to_leave() {
    let m = manager();
    let job = m.admit("a", "speech", fresh(1), &|| u64::MAX).unwrap();
    let finisher = std::thread::spawn({
        let ctl = job.ctl.clone();
        move || {
            while !ctl.is_cancelled() {
                std::thread::sleep(Duration::from_millis(10));
            }
            m.finish("a", job.generation);
        }
    });
    m.shutdown(Duration::from_secs(2));
    finisher.join().unwrap();
    assert!(m.view().job("a").is_none());
}

#[test]
fn a_plan_that_grew_after_admission_must_fit_too() {
    let m = manager();
    let job = m.admit("a", "text", fresh(100), &|| 1000).unwrap();
    assert!(m.grow("a", job.generation, 50, &|| 1000).is_ok());
    assert!(m.grow("a", job.generation, 5000, &|| 1000).is_err());
    assert!(m.admit("b", "speech", fresh(900), &|| 1000).is_err(), "150 owed + 900 > 1000");
    job.ctl.wrote(60);
    assert!(m.grow("a", job.generation, 10, &|| 100).is_ok(), "90 + 10 owed fit the room read now");
}
