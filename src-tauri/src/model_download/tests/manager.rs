//! File: model_download/tests/manager.rs
//! Purpose: Protect the book-keeping of side-by-side downloads (ADR 0013): who may start, who may be deleted, which cancel counts, how file locks hand over and what a quit does.
//! Contents: scenario tests of `Manager` with its own table per test.

use std::sync::mpsc;
use std::time::Duration;

use crate::model_download::state::{Manager, Sizes};

/// A fresh table that lives for the whole test run. Returns it.
fn table() -> &'static Manager {
    Box::leak(Box::new(Manager::new()))
}

/// Sizes for a download of `need` bytes still to write.
fn need(need: u64) -> Sizes {
    Sizes { total: need, need }
}

/// Different models download at once with no queue and no cap, a model already downloading refuses a second start, and finishing frees it.
#[test]
fn models_download_side_by_side_and_a_repeat_start_is_refused() {
    let m = table();
    let free = || u64::MAX;
    let a = m.admit("parakeet", "speech", need(10), &free).expect("first");
    let b = m.admit("whisper", "speech", need(10), &free).expect("second runs alongside");
    let c = m.admit("qwen", "text", need(10), &free).expect("third runs alongside");
    assert!(a.generation < b.generation && b.generation < c.generation);
    assert_eq!(m.admit("parakeet", "speech", need(10), &free).err().as_deref(), Some("This model is already downloading."));
    let view = m.view();
    assert!(view.job("parakeet").is_some() && view.job("whisper").is_some() && view.job("qwen").is_some());
    m.finish("parakeet", a.generation);
    assert!(m.view().job("parakeet").is_none());
    assert!(m.admit("parakeet", "speech", need(10), &free).is_ok());
}

/// A cancel for an old generation never stops a newer download of the same model.
#[test]
fn a_stale_cancel_does_not_stop_the_download_that_replaced_it() {
    let m = table();
    let first = m.admit("model", "speech", need(1), &|| u64::MAX).expect("first");
    m.finish("model", first.generation);
    let second = m.admit("model", "speech", need(1), &|| u64::MAX).expect("second");
    m.cancel("model", first.generation);
    assert!(!second.ctl.is_cancelled());
    m.cancel("model", second.generation);
    assert!(second.ctl.is_cancelled());
    assert!(!first.ctl.is_cancelled());
}

/// Disk room counts what running downloads still owe: a second download that would not fit next to the first is refused until the first has written its bytes or finished.
#[test]
fn a_download_that_does_not_fit_next_to_the_others_is_refused_with_the_shortfall() {
    let m = table();
    let free = || 1_000_000_000u64;
    let first = m.admit("a", "speech", need(600_000_000), &free).expect("fits alone");
    let Err(message) = m.admit("b", "speech", need(600_000_000), &free) else { panic!("a download that does not fit was admitted") };
    assert_eq!(message, "Not enough free disk space: this download needs about 200 MB more.");
    first.ctl.wrote(500_000_000);
    assert!(m.admit("b", "speech", need(600_000_000), &free).is_ok(), "bytes already written no longer count as owed");
}

/// A running download can ask for more room when it finds it has more to write, and is refused when the drive cannot hold it.
#[test]
fn growing_a_reservation_is_checked_against_the_drive() {
    let m = table();
    let job = m.admit("a", "speech", need(100), &|| 1000).expect("admitted");
    assert!(m.grow("a", job.generation, 400, &|| 1000).is_ok());
    assert_eq!(m.grow("a", job.generation, 600, &|| 1000), Err("Not enough free disk space for this download.".to_string()));
}

/// Delete and download exclude each other: a model cannot be deleted while it downloads, nor started while it is being deleted, and text models share one folder so one text model's delete waits for another's download.
#[test]
fn delete_reservations_and_downloads_exclude_each_other() {
    let m = table();
    let free = || u64::MAX;
    let job = m.admit("t1", "text", need(1), &free).expect("download");
    assert_eq!(m.reserve_delete("t1", "text").err().as_deref(), Some("Cancel the download first."));
    assert_eq!(m.reserve_delete("t2", "text").err().as_deref(), Some("Wait for the other download or delete to finish."));
    assert!(m.view().delete_blocked("t2", "text") && !m.view().delete_blocked("s1", "speech"));
    assert!(m.reserve_delete("s1", "speech").is_ok(), "a speech model has its own files");
    m.finish("t1", job.generation);

    let guard = m.reserve_delete("t2", "text").expect("now free");
    assert_eq!(m.admit("t2", "text", need(1), &free).err().as_deref(), Some("This model is being deleted. Try again in a moment."));
    assert!(m.admit("t3", "text", need(1), &free).is_err(), "other text models wait for the shared folder");
    drop(guard);
    assert!(m.admit("t2", "text", need(1), &free).is_ok());
}

/// Two models sharing a file take turns: the second waits for the first to release the path, and a cancel frees a waiter at once.
#[test]
fn a_shared_file_lock_hands_over_and_a_cancel_frees_the_waiter() {
    let m = table();
    let free = || u64::MAX;
    let owner = m.admit("owner", "speech", need(1), &free).expect("owner");
    let waiter = m.admit("waiter", "speech", need(1), &free).expect("waiter");
    let held = m.lock("vad.onnx".into(), &owner.ctl).expect("first lock");

    let (sender, received) = mpsc::channel();
    let ctl = waiter.ctl.clone();
    std::thread::spawn(move || sender.send(m.lock("vad.onnx".into(), &ctl).is_ok()).expect("send"));
    m.cancel("waiter", waiter.generation);
    assert_eq!(received.recv_timeout(Duration::from_secs(30)), Ok(false), "a cancelled waiter gives up");

    let next = m.admit("next", "speech", need(1), &free).expect("next");
    let (sender, received) = mpsc::channel();
    let ctl = next.ctl.clone();
    std::thread::spawn(move || sender.send(m.lock("vad.onnx".into(), &ctl).is_ok()).expect("send"));
    drop(held);
    assert_eq!(received.recv_timeout(Duration::from_secs(30)), Ok(true), "the waiter gets the file once it is released");
}

/// Quitting cancels every download and refuses new ones, so nothing starts while Cue closes.
#[test]
fn shutdown_cancels_running_downloads_and_refuses_new_ones() {
    let m = table();
    let job = m.admit("model", "speech", need(1), &|| u64::MAX).expect("running");
    m.shutdown(Duration::from_millis(20));
    assert!(job.ctl.is_cancelled());
    assert_eq!(m.admit("other", "speech", need(1), &|| u64::MAX).err().as_deref(), Some("Cue is closing."));
}
