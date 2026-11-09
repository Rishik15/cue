//! File: model_download/tests/manager.rs
//! Purpose: Protect the book-keeping of side-by-side downloads (ADR 0013): who may start, who may be deleted, which cancel counts, how file locks hand over and what a quit does.
//! Contents: scenario tests of `Manager` with its own table per test.

use std::sync::mpsc;
use std::time::Duration;

use crate::model_download::ctl::Phase;
use crate::model_download::ledger::Claim;
use crate::model_download::state::{Manager, Outcome, Sizes};

/// A fresh table that lives for the whole test run. Returns it.
fn table() -> &'static Manager {
    Box::leak(Box::new(Manager::new()))
}

/// Sizes for a download of `need` bytes still to write, on a path of its own.
fn need(need: u64) -> Sizes {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    claiming(&format!("path-{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)), need)
}

/// Sizes for a download that still has `need` bytes to write to `key`.
fn claiming(key: &str, need: u64) -> Sizes {
    Sizes { total: need, claims: vec![Claim { key: key.into(), bytes: need }] }
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
    let first = m.admit("a", "speech", claiming("a", 600_000_000), &free).expect("fits alone");
    let Err(message) = m.admit("b", "speech", need(600_000_000), &free) else { panic!("a download that does not fit was admitted") };
    assert_eq!(message, "Not enough free disk space: this download needs about 200 MB more.");
    first.ctl.wrote("a", 500_000_000);
    assert!(m.admit("b", "speech", need(600_000_000), &free).is_ok(), "bytes already written no longer count as owed");
}

/// A running download can ask for more room when it finds it has more to write, and is refused when the drive cannot hold it.
#[test]
fn growing_a_reservation_is_checked_against_the_drive() {
    let m = table();
    let job = m.admit("a", "speech", claiming("a", 100), &|| 1000).expect("admitted");
    assert!(job.ctl.retarget(&[Claim { key: "a".into(), bytes: 500 }], &|| 1000).is_ok());
    assert_eq!(
        job.ctl.retarget(&[Claim { key: "a".into(), bytes: 1100 }], &|| 1000),
        Err("Not enough free disk space for this download.".to_string())
    );
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

/// How a download ended stays on the model until it starts again or is deleted, and a stale generation never writes it.
#[test]
fn the_last_outcome_is_kept_until_the_next_start_or_a_delete() {
    let m = table();
    let free = || u64::MAX;
    let first = m.admit("a", "speech", need(1), &free).expect("first");
    m.conclude("a", first.generation + 9, Outcome::Failed, Some("stale".into()));
    assert!(m.view().last("a").is_none() && m.view().job("a").is_some(), "a stale generation changes nothing");
    m.conclude("a", first.generation, Outcome::Failed, Some("disk error".into()));
    let last = m.view().last("a").cloned().expect("remembered");
    assert_eq!((last.generation, last.outcome, last.error.as_deref()), (first.generation, Outcome::Failed, Some("disk error")));
    m.admit("a", "speech", need(1), &free).expect("again");
    assert!(m.view().last("a").is_none(), "cleared by the next start");

    let second = m.view().job("a").map(|j| j.generation).expect("running");
    m.conclude("a", second, Outcome::Cancelled, None);
    assert!(m.view().last("a").is_some());
    drop(m.reserve_delete("a", "speech").expect("reserved"));
    assert!(m.view().last("a").is_none(), "cleared by a delete");
}

/// The phase shows in the view, starts as fetching and tells its watcher only when it changes.
#[test]
fn the_phase_is_visible_and_announced_once_per_change() {
    let m = table();
    let job = m.admit("a", "speech", need(1), &|| u64::MAX).expect("job");
    let heard = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let count = heard.clone();
    job.ctl.watch_phase(Box::new(move || {
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    assert_eq!(m.view().job("a").map(|j| j.phase), Some(Phase::Fetching));
    job.ctl.set_phase(Phase::Fetching);
    job.ctl.set_phase(Phase::Installing);
    job.ctl.set_phase(Phase::Installing);
    assert_eq!(m.view().job("a").map(|j| j.phase.name()), Some("installing"));
    assert_eq!(heard.load(std::sync::atomic::Ordering::SeqCst), 1);
}

/// A cancel runs every registered interrupt once; one registered after the cancel runs at once; one that was unregistered never runs.
#[test]
fn cancel_runs_the_registered_interrupts() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    let m = table();
    let job = m.admit("a", "speech", need(1), &|| u64::MAX).expect("job");
    let hits = Arc::new(AtomicU32::new(0));
    let bump = |hits: &Arc<AtomicU32>| {
        let hits = hits.clone();
        Box::new(move || {
            hits.fetch_add(1, Ordering::SeqCst);
        }) as crate::model_download::ctl::Interrupt
    };
    job.ctl.register(bump(&hits));
    let skipped = job.ctl.register(bump(&hits));
    job.ctl.unregister(skipped);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    m.cancel("a", job.generation);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    job.ctl.register(bump(&hits));
    assert_eq!(hits.load(Ordering::SeqCst), 2, "registered after the cancel: runs at once");
    m.cancel("a", job.generation);
    assert_eq!(hits.load(Ordering::SeqCst), 2, "a second cancel runs nothing again");
}

/// A download cancelled by the time it concludes can never publish success.
#[test]
fn a_cancelled_download_never_concludes_as_succeeded() {
    let m = table();
    let job = m.admit("a", "speech", need(1), &|| u64::MAX).expect("job");
    m.cancel("a", job.generation);
    let (_, outcome) = m.conclude("a", job.generation, Outcome::Succeeded, None);
    assert_eq!(outcome, Outcome::Cancelled);
    assert_eq!(m.view().last("a").map(|l| l.outcome), Some(Outcome::Cancelled));
    let other = m.admit("b", "speech", need(1), &|| u64::MAX).expect("other");
    assert_eq!(m.conclude("b", other.generation, Outcome::Succeeded, None).1, Outcome::Succeeded);
}

/// Once a download is committed (it started publishing), a cancel is ignored and a success stays a success; a cancel that came first wins the commit.
#[test]
fn the_commit_point_decides_between_a_cancel_and_a_publish() {
    let m = table();
    let committed = m.admit("a", "speech", need(1), &|| u64::MAX).expect("a");
    assert!(committed.ctl.commit());
    m.cancel("a", committed.generation);
    assert!(!committed.ctl.is_cancelled());
    assert_eq!(m.conclude("a", committed.generation, Outcome::Succeeded, None).1, Outcome::Succeeded);

    let lost = m.admit("b", "speech", need(1), &|| u64::MAX).expect("b");
    m.cancel("b", lost.generation);
    assert!(!lost.ctl.commit(), "a cancel that came first wins");
    assert_eq!(m.conclude("b", lost.generation, Outcome::Succeeded, None).1, Outcome::Cancelled);
}

/// A quit does not cancel a committed download, which finishes publishing by itself.
#[test]
fn shutdown_leaves_a_committed_download_alone() {
    let m = table();
    let job = m.admit("a", "speech", need(1), &|| u64::MAX).expect("a");
    assert!(job.ctl.commit());
    m.shutdown(Duration::from_millis(20));
    assert!(!job.ctl.is_cancelled());
}

/// A re-plan that changes the total shows in the view, so the page never sees `done` beyond `total`.
#[test]
fn a_replanned_total_shows_in_the_view() {
    let m = table();
    let job = m.admit("a", "speech", need(100), &|| u64::MAX).expect("a");
    assert_eq!(m.view().job("a").map(|j| j.total), Some(100));
    job.ctl.set_total(250);
    assert_eq!(m.view().job("a").map(|j| j.total), Some(250));
}

/// A new handle can be shared again for something that outlives a call.
#[test]
fn a_handle_can_be_shared_again() {
    let m = table();
    let job = m.admit("a", "speech", need(1), &|| u64::MAX).expect("a");
    assert!(job.ctl.shared().is_some_and(|c| std::sync::Arc::ptr_eq(&c, &job.ctl)));
}
