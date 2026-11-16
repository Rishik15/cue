//! File: snippets/tests/phase.rs
//! Purpose: Protect the expansion stages: keys typed while an expansion prepares are settled at the seal, keys typed after the seal are kept in order and never cleared, and a failed delivery never replays them.
//! Contents: scenario tests of `Phase`.

use crate::platform::Typed::{self, Backspace, Char, Reset};
use crate::snippets::expand::phase::{Late, Phase};

/// A phase that has begun and received `keys`.
fn preparing(keys: &[Typed]) -> Phase {
    let mut phase = Phase::default();
    assert!(phase.begin());
    keys.iter().for_each(|k| assert!(phase.push(*k)));
    phase
}

/// A second begin while busy changes nothing.
#[test]
fn begin_while_busy_is_refused() {
    let mut phase = preparing(&[Char('a')]);
    assert!(!phase.begin());
    assert_eq!(phase, Phase::Preparing(vec![Char('a')]));
}

/// Keys collected before the seal come back in order as the retyped text.
#[test]
fn seal_returns_the_keys_in_order() {
    let mut phase = preparing(&[Char('h'), Char('i')]);
    assert_eq!(phase.seal().as_deref(), Some("hi"));
    assert_eq!(phase, Phase::Committing(Vec::new()));
}

/// Backspace removes one collected character.
#[test]
fn backspace_pops_one_character() {
    let mut phase = preparing(&[Char('a'), Char('b'), Backspace]);
    assert_eq!(phase.seal().as_deref(), Some("a"));
}

/// Backspace into the trigger, Enter and a reset all stop the expansion, and the phase still seals so later keys are kept.
#[test]
fn abort_keys_stop_the_expansion_and_still_seal() {
    for keys in [vec![Backspace], vec![Char('x'), Char('\n')], vec![Reset]] {
        let mut phase = preparing(&keys);
        assert_eq!(phase.seal(), None);
        assert_eq!(phase, Phase::Committing(Vec::new()));
    }
}

/// A key after the seal is held, and finishing hands it back instead of clearing it.
#[test]
fn a_late_key_survives_the_cleanup() {
    let mut phase = preparing(&[Char('a')]);
    phase.seal();
    assert!(phase.push(Char('z')));
    assert_eq!(phase.finish(true), Late::Replay(vec![Char('z')]));
    assert_eq!(phase, Phase::Idle);
}

/// After a failed delivery late keys are counted, not replayed.
#[test]
fn a_failed_delivery_withholds_late_keys() {
    let mut phase = preparing(&[]);
    phase.seal();
    phase.push(Char('z'));
    phase.push(Char('y'));
    assert_eq!(phase.finish(false), Late::Withheld(2));
    assert_eq!(phase, Phase::Idle);
}

/// A worker that never sealed (declined, spawn failed) still gives its keys back.
#[test]
fn finishing_without_a_seal_keeps_the_keys() {
    let mut phase = preparing(&[Char('q')]);
    assert_eq!(phase.finish(true), Late::Replay(vec![Char('q')]));
}

/// Idle ignores keys and sealing, and a stale finish cannot invent any.
#[test]
fn idle_is_inert() {
    let mut phase = Phase::default();
    assert!(!phase.push(Char('a')));
    assert_eq!(phase.seal(), None);
    assert_eq!(phase, Phase::Idle);
    assert_eq!(phase.finish(true), Late::Replay(Vec::new()));
}

/// A second seal in Committing does not take the late keys.
#[test]
fn sealing_twice_leaves_late_keys_alone() {
    let mut phase = preparing(&[]);
    phase.seal();
    phase.push(Char('z'));
    assert_eq!(phase.seal(), None);
    assert_eq!(phase, Phase::Committing(vec![Char('z')]));
}
