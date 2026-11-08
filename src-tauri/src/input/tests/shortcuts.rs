//! File: input/tests/shortcuts.rs
//! Purpose: Protect global shortcuts through the scripted keyboard: hold and press shortcuts, the latch of a held key after its modifier is released, interleaved typing, on/off switches, lost releases after a session change, Cue's own and foreign injected keys, and a full effect queue.
//! Contents: scenario tests of pass or swallow per edge and of the active actions.

use super::{fake, CA};
use crate::input::core::{Origin, Verdict};
use crate::input::ledger::Slot;
use crate::input::{Key, Mods};

const PASS: Verdict = Verdict::Pass;
const SWALLOW: Verdict = Verdict::Swallow;

/// Ctrl+Space holds dictation: the Space is swallowed, the Ctrl passes, release lets go.
#[test]
fn a_hold_shortcut_starts_and_releases() {
    let f = fake();
    assert_eq!(f.mod_down(Mods::CTRL_L), PASS);
    assert_eq!(f.down(Key::Space), SWALLOW);
    assert_eq!(f.active(), vec![0]);
    assert_eq!(f.up(Key::Space), SWALLOW);
    assert!(f.active().is_empty());
    assert_eq!(f.mod_up(Mods::CTRL_L), PASS);
    assert_eq!(f.passed(), vec!["+mod4", "-mod4"]);
}

/// Ctrl released first: the held Space keeps being swallowed (repeats and release) and never restarts the shortcut.
#[test]
fn modifier_first_release_keeps_the_key_latched() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::Space);
    assert_eq!(f.mod_up(Mods::CTRL_L), PASS);
    assert!(f.active().is_empty(), "letting go of Ctrl ends the hold");
    assert_eq!(f.down(Key::Space), SWALLOW, "repeat");
    assert_eq!(f.up(Key::Space), SWALLOW);
    assert!(f.log.lock().unwrap().actions.iter().filter(|a| a.1).count() == 1, "one start only");
    assert_eq!(f.down(Key::Space), PASS, "a fresh Space is just a space");
}

/// Another key typed while a swallowed Space is held does not make the Space's repeats leak (the old latch pruning bug).
#[test]
fn interleaved_typing_does_not_unlatch() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::Space);
    f.mod_up(Mods::CTRL_L);
    assert_eq!(f.down(Key::A), PASS);
    assert_eq!(f.up(Key::A), PASS);
    assert_eq!(f.down(Key::Space), SWALLOW);
    assert_eq!(f.up(Key::Space), SWALLOW);
}

/// A press shortcut runs once on key down and swallows its key.
#[test]
fn a_press_shortcut_runs_on_key_down() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    f.mod_down(Mods::ALT_L);
    assert_eq!(f.down(Key::G), SWALLOW);
    assert!(f.log.lock().unwrap().actions.contains(&(2, true)));
    assert_eq!(f.up(Key::G), SWALLOW);
    let _ = CA;
}

/// A bare F-key shortcut is swallowed and released when a session change loses its key-up.
#[test]
fn a_lost_f_key_release_is_recovered() {
    let f = fake();
    assert_eq!(f.down(Key::F8), SWALLOW);
    assert_eq!(f.active(), vec![3]);
    f.session();
    assert!(f.active().is_empty());
    assert_eq!(f.up(Key::F8), PASS, "the lost up is forgotten, not swallowed forever");
}

/// Turning a shortcut off releases its hold and frees its keys.
#[test]
fn turning_a_shortcut_off_releases_and_passes() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::Space);
    f.store.set_enabled("dictation", false);
    f.input.refresh_enabled();
    assert!(f.active().is_empty());
    f.up(Key::Space);
    f.mod_up(Mods::CTRL_L);
    f.mod_down(Mods::CTRL_L);
    assert_eq!(f.down(Key::Space), PASS);
}

/// Cue's own marked keys and the AltGr phantom are invisible; another program's injected key counts as physical.
#[test]
fn marked_keys_are_invisible_and_foreign_keys_count() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::Space);
    assert_eq!(f.edge_from(Slot::Mod(Mods::CTRL_L), false, Origin::Cue), PASS);
    assert_eq!(f.edge_from(Slot::Mod(Mods::CTRL_L), false, Origin::Phantom), PASS);
    assert_eq!(f.active(), vec![0], "still held: marked and phantom edges changed nothing");
    assert_eq!(f.edge_from(Slot::Mod(Mods::CTRL_L), false, Origin::Physical), PASS);
    assert!(f.active().is_empty());
}

/// Escape and typing reach the observers without being swallowed.
#[test]
fn escape_and_typing_are_observed_and_pass() {
    let f = fake();
    assert_eq!(f.down(Key::Escape), PASS);
    assert_eq!(f.down(Key::A), PASS);
    f.mod_down(Mods::CTRL_L);
    f.down(Key::Space);
    let log = f.log.lock().unwrap();
    assert_eq!((log.escapes, log.typing), (1, 2), "Escape counts as typing too; the Space shortcut key does not");
}

/// When an effect cannot be queued the key passes, the hold is released and nothing stays captured.
#[test]
fn a_full_queue_fails_open() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    f.refuse(true);
    assert_eq!(f.down(Key::Space), PASS);
    f.refuse(false);
    assert!(f.active().is_empty());
    assert_eq!(f.up(Key::Space), PASS);
}

/// Swallowing a key while Alt is held queues one inert key tap, once per hold, so letting go of Alt opens no menu; Ctrl alone needs none.
#[test]
fn a_swallowed_key_under_alt_gets_one_menu_mask() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    f.mod_down(Mods::ALT_L);
    f.down(Key::G);
    f.up(Key::G);
    f.down(Key::G);
    assert_eq!(f.log.lock().unwrap().masks, 1);
    f.up(Key::G);
    f.mod_up(Mods::ALT_L);
    f.mod_down(Mods::ALT_L);
    f.down(Key::G);
    assert_eq!(f.log.lock().unwrap().masks, 2, "a new hold gets a new mask");
    let ctrl_only = fake();
    ctrl_only.mod_down(Mods::CTRL_L);
    ctrl_only.down(Key::Space);
    assert_eq!(ctrl_only.log.lock().unwrap().masks, 0);
}
