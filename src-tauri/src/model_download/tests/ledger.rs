//! File: model_download/tests/ledger.rs
//! Purpose: Protect the disk books of downloads (ADR 0013) by feeding ops in the order a race would produce them: no threads, no disk.
//! Contents: scenario tests of `step` over a `Ledger`.

use crate::model_download::ledger::{step, Claim, Ledger, Op, Short};

/// A claim of `bytes` on `key`.
fn claim(key: &str, bytes: u64) -> Claim {
    Claim { key: key.into(), bytes }
}

/// Admits `job` with `claims` against `free`.
fn admit(ledger: &mut Ledger, job: (&'static str, u64), claims: &[Claim], free: u64) -> Result<(), Short> {
    step(ledger, Op::Admit { job, claims, free })
}

/// Two models that unpack the same archive count it once: with room for exactly one archive both start, and a third download that needs one byte more is refused with that shortfall.
#[test]
fn a_shared_archive_is_counted_once() {
    let mut books = Ledger::new();
    let (a, b, c) = (("a", 1), ("b", 2), ("c", 3));
    assert_eq!(admit(&mut books, a, &[claim("archive", 1000)], 1000), Ok(()));
    assert_eq!(admit(&mut books, b, &[claim("archive", 1000)], 1000), Ok(()));
    assert_eq!(books.debt(), 1000);
    assert_eq!(admit(&mut books, c, &[claim("other", 1)], 1000), Err(Short(1)));
    assert_eq!(books.debt(), 1000, "a refusal changes nothing");
}

/// A partial thrown away is owed again before the delete frees its room, so an admit that comes between the two sees the debt and is refused.
#[test]
fn a_discard_cannot_let_another_download_take_its_room() {
    let mut books = Ledger::new();
    let (a, b) = (("a", 1), ("b", 2));
    assert_eq!(admit(&mut books, a, &[claim("file", 100)], 150), Ok(()));
    assert_eq!(step(&mut books, Op::Wrote { key: "file", bytes: 100 }), Ok(()));
    assert_eq!(books.debt(), 0);
    assert_eq!(step(&mut books, Op::Lose { key: "file", bytes: 100 }), Ok(()));
    assert_eq!(
        admit(&mut books, b, &[claim("other", 100)], 150),
        Err(Short(50)),
        "the room the delete is about to free is already spoken for"
    );
}

/// The owner of a shared path cancels half way: the waiter still holds the reservation, and re-planning from the disk as it is now keeps it (no false refusal on a nearly full disk).
#[test]
fn a_waiter_inherits_the_reservation_when_its_owner_cancels() {
    let mut books = Ledger::new();
    let (owner, waiter) = (("owner", 1), ("waiter", 2));
    assert_eq!(admit(&mut books, owner, &[claim("archive", 1000)], 1200), Ok(()));
    assert_eq!(admit(&mut books, waiter, &[claim("archive", 1000)], 1200), Ok(()));
    assert_eq!(step(&mut books, Op::Wrote { key: "archive", bytes: 400 }), Ok(()));
    assert_eq!(step(&mut books, Op::Release { job: owner }), Ok(()));
    assert!(!books.holds(owner, "archive") && books.holds(waiter, "archive"));
    assert_eq!(books.reserved("archive"), 600);
    assert_eq!(step(&mut books, Op::Retarget { job: waiter, claims: &[claim("archive", 600)], free: 650 }), Ok(()));
    assert_eq!(books.debt(), 600);
}

/// The owner discards the partial before handing over: the waiter's re-plan needs the whole archive again, and is refused only when the drive truly cannot hold it.
#[test]
fn a_handover_after_a_discard_is_checked_against_the_drive() {
    let mut books = Ledger::new();
    let (owner, waiter) = (("owner", 1), ("waiter", 2));
    assert_eq!(admit(&mut books, owner, &[claim("archive", 1000)], 1200), Ok(()));
    assert_eq!(admit(&mut books, waiter, &[claim("archive", 1000)], 1200), Ok(()));
    assert_eq!(step(&mut books, Op::Wrote { key: "archive", bytes: 400 }), Ok(()));
    assert_eq!(step(&mut books, Op::Lose { key: "archive", bytes: 400 }), Ok(()));
    assert_eq!(step(&mut books, Op::Release { job: owner }), Ok(()));
    assert_eq!(step(&mut books, Op::Retarget { job: waiter, claims: &[claim("archive", 1000)], free: 1000 }), Ok(()));
    let mut full = books.clone();
    assert_eq!(step(&mut full, Op::Retarget { job: waiter, claims: &[claim("archive", 1500)], free: 1000 }), Err(Short(500)));
    assert_eq!(full, books, "a refused re-plan changes nothing");
}

/// When the owner finishes and another model's work already installed the file, the waiter's re-plan frees the reservation; releasing the last holder empties the books.
#[test]
fn finished_work_is_released_and_the_books_empty_out() {
    let mut books = Ledger::new();
    let (a, b) = (("a", 1), ("b", 2));
    assert_eq!(admit(&mut books, a, &[claim("archive", 1000), claim("archive#unpack", 2000)], 5000), Ok(()));
    assert_eq!(admit(&mut books, b, &[claim("archive", 1000), claim("archive#unpack", 2000)], 5000), Ok(()));
    assert_eq!(step(&mut books, Op::Release { job: a }), Ok(()));
    assert_eq!(step(&mut books, Op::Retarget { job: b, claims: &[claim("archive", 0), claim("archive#unpack", 0)], free: 5000 }), Ok(()));
    assert_eq!(books.debt(), 0);
    assert_eq!(step(&mut books, Op::Release { job: b }), Ok(()));
    assert_eq!(books, Ledger::new());
}

/// Writes and losses for a path nobody relies on are ignored, and a write can never take the reservation below zero.
#[test]
fn accounting_for_unknown_paths_and_overwrites_is_harmless() {
    let mut books = Ledger::new();
    assert_eq!(step(&mut books, Op::Wrote { key: "nothing", bytes: 5 }), Ok(()));
    assert_eq!(step(&mut books, Op::Lose { key: "nothing", bytes: 5 }), Ok(()));
    assert_eq!(books, Ledger::new());
    assert_eq!(admit(&mut books, ("a", 1), &[claim("f", 10)], 100), Ok(()));
    assert_eq!(step(&mut books, Op::Wrote { key: "f", bytes: 99 }), Ok(()));
    assert_eq!(books.reserved("f"), 0);
}

/// A re-plan that only takes bytes off the debt is never refused, even when the drive now holds less than the debt (it filled up meanwhile).
#[test]
fn a_shrinking_retarget_is_never_refused() {
    let mut books = Ledger::new();
    let job = ("a", 1);
    assert_eq!(admit(&mut books, job, &[claim("file", 1000), claim("file#unpack", 500)], 5000), Ok(()));
    assert_eq!(step(&mut books, Op::Retarget { job, claims: &[claim("file", 400), claim("file#unpack", 500)], free: 100 }), Ok(()));
    assert_eq!(books.debt(), 900);
    assert_eq!(
        step(&mut books, Op::Retarget { job, claims: &[claim("file", 400), claim("file#unpack", 500)], free: 100 }),
        Ok(()),
        "an unchanged plan is not refused either"
    );
    assert_eq!(
        step(&mut books, Op::Retarget { job, claims: &[claim("file", 0), claim("file#unpack", 600)], free: 0 }),
        Ok(()),
        "growing one path while shrinking more is a net shrink"
    );
    assert_eq!(books.debt(), 600);
}
