//! File: input/tests/bare.rs
//! Purpose: Protect the no-shortcut input the debug insert test starts: every key passes to the app, and on Windows it installs the hook and publishes the shared state `lift` needs.
//! Contents: tests of `bare::BARE` through the scripted keyboard and of `start_bare`.

use crate::input::bare::BARE;
use crate::input::core::Verdict;
use crate::input::fake::Fake;
use crate::input::{Key, Mods};

/// Cue's default shortcut keys and Escape all pass: the bare input claims nothing.
#[test]
fn bare_input_passes_every_key() {
    let f = Fake::new(&BARE);
    assert_eq!(f.mod_down(Mods::CTRL_L), Verdict::Pass);
    assert_eq!(f.down(Key::Space), Verdict::Pass);
    assert_eq!(f.up(Key::Space), Verdict::Pass);
    assert_eq!(f.mod_up(Mods::CTRL_L), Verdict::Pass);
    assert_eq!(f.down(Key::Escape), Verdict::Pass);
    assert!(f.active().is_empty());
}

/// Starting the bare input installs the keyboard hook and publishes the shared state, so `lift` no longer says the keyboard is not running.
#[cfg(windows)]
#[test]
fn start_bare_publishes_the_shared_state() {
    let input = crate::input::start_bare().expect("the keyboard hook installs");
    assert!(crate::input::shared::global().is_some());
    input.shutdown();
}
