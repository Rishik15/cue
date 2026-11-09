//! File: input/tests/cost.rs
//! Purpose: Measure what one key edge costs the hook thread: an ordinary key (the common case, in every app), a shortcut press and release, and a modifier chord, as a mean and a worst case in microseconds. Ignored by default because timing numbers do not hold on shared runners; run it to compare before and after a change to the callback path.
//! Contents: `edge_cost`, run with `cargo test --lib input::tests::cost -- --ignored --nocapture`.

use std::time::Instant;

use super::fake;
use crate::input::{Key, Mods};

const ROUNDS: usize = 20_000;

/// Times `step` over `ROUNDS` rounds and prints the mean and the worst round in microseconds. Inputs: a label and the work of one round.
fn time(label: &str, mut step: impl FnMut()) {
    let mut worst = 0u128;
    let begun = Instant::now();
    for _ in 0..ROUNDS {
        let at = Instant::now();
        step();
        worst = worst.max(at.elapsed().as_nanos());
    }
    let mean = begun.elapsed().as_nanos() / ROUNDS as u128;
    println!("{label:<34} mean {:>7.2} us   worst {:>8.2} us", mean as f64 / 1000.0, worst as f64 / 1000.0);
}

/// Prints the cost of an ordinary key, a shortcut and a modifier chord through the whole fake keyboard (reducer, ledger, matcher, effects flush).
#[test]
#[ignore = "timing numbers; run by hand"]
fn edge_cost() {
    let f = fake();
    time("ordinary key down+up", || {
        f.down(Key::A);
        f.up(Key::A);
    });
    time("shortcut F8 down+up", || {
        f.down(Key::F8);
        f.up(Key::F8);
    });
    time("Ctrl+Alt+G chord down+up", || {
        f.mod_down(Mods::CTRL_L);
        f.mod_down(Mods::ALT_L);
        f.down(Key::G);
        f.up(Key::G);
        f.mod_up(Mods::ALT_L);
        f.mod_up(Mods::CTRL_L);
    });
}
