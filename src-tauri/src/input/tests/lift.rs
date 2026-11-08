//! File: input/tests/lift.rs
//! Purpose: Protect the lift plan: which key events lift and restore a held set, and the menu mask around Alt and Win.
//! Contents: tests of `lift::plan`.

use crate::input::lift::plan;
use crate::input::Mods;

/// Ctrl+Shift lifts both and restores them in reverse order, with no mask.
#[test]
fn ctrl_shift_lift_and_restore_in_reverse() {
    let (lift, restore) = plan(Mods::CTRL_L | Mods::SHIFT_L);
    let vks = |b: &crate::input::KeyBatch| b.0.iter().map(|s| (s.vk, s.up)).collect::<Vec<_>>();
    assert_eq!(vks(&lift), vec![(0xA0, true), (0xA2, true)]);
    assert_eq!(vks(&restore), vec![(0xA2, false), (0xA0, false)]);
}

/// Alt and Win get a mask tap before the lift and after the restore so the shell opens no menu.
#[test]
fn alt_gets_a_menu_mask_around_the_lift() {
    let (lift, restore) = plan(Mods::ALT_L);
    assert_eq!(lift.0.first().map(|s| (s.vk, s.up)), Some((0xE8, false)));
    assert_eq!(lift.0.last().map(|s| (s.vk, s.up)), Some((0xA4, true)));
    assert_eq!(restore.0.last().map(|s| (s.vk, s.up)), Some((0xE8, true)));
    assert!(plan(Mods::NONE).0 .0.is_empty());
}
