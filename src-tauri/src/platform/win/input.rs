//! Purpose: Let Cue send its own keystrokes (Ctrl+C to read a selection, Ctrl+V, typed text) while the user is still holding the shortcut that started them, so a shortcut acts the moment it is pressed.
//! The modifiers that are down are lifted (key-up), Cue's keys go out, and the modifiers are put back (key-down) only if they are still physically held. Every key Cue injects carries `handy_keys::CUE_INPUT_MARK`, so the keyboard hook ignores it.
//! Contents: Lift — a guard that lifts the held modifiers and restores them on drop; plan — the pure list of key events for a held set; MARK — the marker for injected input.

use std::sync::{Mutex, MutexGuard};

use handy_keys::{held_modifiers, set_lifting, Modifiers, CUE_INPUT_MARK};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use super::{elevation, insert};

/// The marker every injected key carries.
pub const MARK: usize = CUE_INPUT_MARK;

/// Unassigned virtual key the shell ignores; a lone Alt or Win press-and-release would open a menu, so one of these follows a restored Alt or Win.
const MENU_MASK: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

/// One key event of a lift or restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub vk: VIRTUAL_KEY,
    pub up: bool,
    pub extended: bool,
}

/// Every side-specific modifier key, with the flag the hook tracks it by.
const KEYS: [(Modifiers, VIRTUAL_KEY, bool); 8] = [
    (Modifiers::CMD_LEFT, VK_LWIN, true),
    (Modifiers::CMD_RIGHT, VK_RWIN, true),
    (Modifiers::SHIFT_LEFT, VK_LSHIFT, false),
    (Modifiers::SHIFT_RIGHT, VK_RSHIFT, false),
    (Modifiers::CTRL_LEFT, VK_LCONTROL, false),
    (Modifiers::CTRL_RIGHT, VK_RCONTROL, true),
    (Modifiers::OPT_LEFT, VK_LMENU, false),
    (Modifiers::OPT_RIGHT, VK_RMENU, true),
];

/// The key events that lift `held` and the ones that put it back. Alt and Win get a menu mask before they are lifted and after they are pressed again: the shell opens a menu when Alt or Win goes down and up with no other key in between, and a lone modifier-only shortcut has no swallowed key to prevent that.
pub fn plan(held: Modifiers) -> (Vec<Step>, Vec<Step>) {
    let down: Vec<_> = KEYS.iter().filter(|(flag, ..)| held.contains(*flag)).collect();
    let mask = held.intersects(Modifiers::OPT | Modifiers::CMD);
    let tap = || [Step { vk: MENU_MASK, up: false, extended: false }, Step { vk: MENU_MASK, up: true, extended: false }];
    let mut lift: Vec<Step> = if mask { tap().to_vec() } else { Vec::new() };
    lift.extend(down.iter().map(|(_, vk, extended)| Step { vk: *vk, up: true, extended: *extended }));
    let mut restore: Vec<Step> = down.iter().rev().map(|(_, vk, extended)| Step { vk: *vk, up: false, extended: *extended }).collect();
    if mask {
        restore.extend(tap());
    }
    (lift, restore)
}

/// The modifiers Windows reports down right now (before any lift, the logical state is the physical one).
fn down_now() -> Modifiers {
    KEYS.iter()
        .filter(|(_, vk, _)| (unsafe { GetAsyncKeyState(vk.0 as i32) }) as u16 & 0x8000 != 0)
        .fold(Modifiers::empty(), |all, (flag, ..)| all | *flag)
}

fn inputs(steps: &[Step]) -> Vec<INPUT> {
    steps
        .iter()
        .map(|s| {
            let flags = if s.up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }
                | if s.extended { KEYEVENTF_EXTENDEDKEY } else { KEYBD_EVENT_FLAGS(0) };
            insert::key(s.vk, 0, flags)
        })
        .collect()
}

/// One lift at a time in the whole process: a second thread must not read the keys as up (they are lifted) or put them back while the first is still typing.
static LIFTING: Mutex<()> = Mutex::new(());

/// While alive, the modifiers the user is holding are lifted. Dropping it puts back the ones still held; one the user let go of meanwhile is released again so nothing sticks.
pub struct Lift {
    held: Modifiers,
    _one_at_a_time: MutexGuard<'static, ()>,
}

impl Lift {
    /// Nothing is lifted when no modifier is down, or when the focused app is elevated (Windows would drop the events and leave the keys stuck). What is lifted is what Windows reports down, so a hook that stopped working still gets Ctrl+C through;
    /// only keys the hook also sees held are put back, so a stale hook can never press a key that is not there.
    pub fn new() -> Lift {
        let guard = LIFTING.lock().unwrap_or_else(|e| e.into_inner());
        let mut held = if elevation::target_blocks_input() { Modifiers::empty() } else { down_now() };
        if !held.is_empty() {
            set_lifting(true);
            if insert::send(&inputs(&plan(held).0)).is_err() {
                // Nothing was lifted, so there is nothing to put back; pressing the keys again would double them.
                set_lifting(false);
                held = Modifiers::empty();
            }
        }
        Lift { held, _one_at_a_time: guard }
    }
}

impl Drop for Lift {
    fn drop(&mut self) {
        if self.held.is_empty() {
            return;
        }
        let mut still = self.held & held_modifiers(); // the hook's own tracking says which are physically down now
        let _ = insert::send(&inputs(&plan(still).1));
        // A key the user let go of while it was being put back would stay logically down: look again after 4 ms and 16 ms and release it.
        for wait_ms in [4, 16] {
            std::thread::sleep(std::time::Duration::from_millis(wait_ms));
            let gone = still & !held_modifiers();
            if !gone.is_empty() {
                let _ = insert::send(&inputs(&plan(gone).0));
                still &= !gone;
            }
        }
        set_lifting(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn describe(steps: &[Step]) -> Vec<(u16, bool)> {
        steps.iter().map(|s| (s.vk.0, s.up)).collect()
    }

    #[test]
    fn nothing_held_means_nothing_to_do() {
        let (lift, restore) = plan(Modifiers::empty());
        assert!(lift.is_empty() && restore.is_empty());
    }

    #[test]
    fn ctrl_alt_is_lifted_and_restored_in_reverse_with_a_menu_mask_on_both_sides() {
        let (lift, restore) = plan(Modifiers::CTRL_LEFT | Modifiers::OPT_LEFT);
        assert_eq!(describe(&lift), vec![(0xE8, false), (0xE8, true), (VK_LCONTROL.0, true), (VK_LMENU.0, true)]);
        assert_eq!(describe(&restore), vec![(VK_LMENU.0, false), (VK_LCONTROL.0, false), (0xE8, false), (0xE8, true)]);
    }

    #[test]
    fn ctrl_alone_needs_no_menu_mask() {
        let (lift, restore) = plan(Modifiers::CTRL_LEFT);
        assert_eq!(describe(&lift), vec![(VK_LCONTROL.0, true)]);
        assert_eq!(describe(&restore), vec![(VK_LCONTROL.0, false)]);
    }

    #[test]
    fn right_side_keys_are_extended() {
        let (lift, _) = plan(Modifiers::CTRL_RIGHT | Modifiers::OPT_RIGHT | Modifiers::CMD_LEFT | Modifiers::SHIFT_LEFT);
        assert!(lift.iter().filter(|s| s.vk == VK_RCONTROL || s.vk == VK_RMENU || s.vk == VK_LWIN).all(|s| s.extended));
        assert!(!lift.iter().any(|s| s.vk == VK_LSHIFT && s.extended));
    }
}
