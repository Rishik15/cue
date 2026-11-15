//! File: selection.rs
//! Purpose: Read the text selected in the focused app by copying it, without disturbing what the user had on the clipboard, without a fixed sleep and without trusting a clipboard change that is not provably the target's copy.
//! Contents: capture — one transaction: refuse a password field or an administrator app, check the focus is still the target, return at once when the field says nothing is selected, listen for clipboard changes, then admit and borrow the clipboard (in that order, so no foreign copy can fall between), lift the held modifiers, send Ctrl+C, wait for the change message or one deadline, read the copy and its owner in one open, accept it only if every sign agrees (`origin`), put the old clipboard back against that copy's own sequence number.
//! copy_and_look — the final focus check, the copy and the read; bind — the field the result belongs to; settle — what a read means; uia_fallback — the selection as UI Automation knows it when the clipboard cannot be borrowed.

use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, VK_C, VK_CONTROL};

use super::app_name::foreground_app;
use super::borrow::{Borrow, Ours};
use super::clipboard::{look_within, sequence, Looked, HTML_BYTES, TEXT_BYTES};
use super::clipboard_tx::origin::{accepted, descends_from, Signs};
use super::clipboard_tx::{self, Owner};
use super::context::{field_state, selection_in, FieldState, Session};
use super::elevation;
use super::focus::{foreground, target_now};
use super::input::Lift;
use super::insert::{ctrl_shift_chord, key_chord, send};
use crate::platform::{Capture, Copied, Target};

/// How long the target gets to answer Ctrl+C: apps answer within milliseconds, a few (spreadsheets, remote desktops) render lazily and a busy machine is slower. Only an app that never answers waits the full time, and that is reported as unknown, not as nothing selected.
const COPY_TIMEOUT: Duration = Duration::from_millis(2000);
/// The longest selection read through UI Automation when the clipboard cannot be borrowed.
const MAX_UIA_CHARS: usize = 200_000;

/// Copies the selection of `target` and returns it. The window level is what is checked here (a freshly built web window changes its focused element a moment after it opens, which made a valid selection look like another place); the field that has the keyboard when Ctrl+C goes out is bound into the returned `Copied.target`, and delivery checks that token with the same `Target::same_as`.  `cancelled` is asked before the copy is sent and before its result is used. `Empty` at once when the field says nothing is selected, `Unknown` when the target did not answer or the change is not provably its copy (nothing is used, nothing is restored over it), `Refused` with a line for the user when the field is a password, the app is elevated, the selection is too long, or the clipboard cannot be borrowed and UI Automation does not know the text.
pub fn capture(target: Target, cancelled: &dyn Fn() -> bool) -> Capture {
    let target = Target { runtime_id: None, key: String::new(), ..target };
    let Ok(_tx) = clipboard_tx::enter() else { return Capture::Refused("Cue is busy with another clipboard operation. Try again.".into()) };
    if elevation::target_blocks_input() {
        return Capture::Refused("This app runs as administrator, so Cue cannot read its selection.".into());
    }
    let begun = Instant::now();
    let session = Session::open();
    if !on_target(&target) {
        crate::log::note("selection: the focus is no longer in the window the shortcut was pressed in");
        return Capture::Unknown;
    }
    let said_empty = match check_field(session.as_ref(), &target) {
        Ok(said_empty) => said_empty,
        Err(done) => return *done,
    };
    let owner = match Owner::new() {
        Ok(owner) => owner,
        Err(e) => return Capture::Refused(e),
    };
    let listening = match owner.listen() {
        Ok(listening) => listening,
        Err(e) => return Capture::Refused(e),
    };
    let borrow = match Borrow::begin() {
        Ok(borrow) => borrow,
        Err(refused) => return uia_fallback(session.as_ref(), refused.reason(), &target),
    };
    if cancelled() {
        return Capture::Cancelled;
    }
    let found = copy_and_look(&owner, &target, borrow.sequence());
    drop(listening);
    crate::log::note(&format!(
        "selection: {} in {} ms",
        if matches!(found, Ok(Some(_))) { "copied" } else { "none" },
        begun.elapsed().as_millis()
    ));
    match found {
        Err(why) => Capture::Refused(why),
        Ok(None) if said_empty => Capture::Empty,
        Ok(None) => Capture::Unknown,
        Ok(Some((looked, signs, bound))) => {
            let settled = settle(borrow, &owner, looked, &signs, bound);
            if cancelled() {
                Capture::Cancelled
            } else {
                settled
            }
        }
    }
}

/// Looks at the focused field before anything is sent. Err with the final answer when the field is a password or says nothing is selected and the window is one whose answer stands; Ok(true) when it says nothing is selected but the copy is tried anyway (`trusts_empty`); Ok(false) otherwise.
fn check_field(session: Option<&Session>, target: &Target) -> Result<bool, Box<Capture>> {
    let state = field_state(session);
    crate::log::note(&format!("selection: field {state:?} in {} ({})", foreground_app(), target.class));
    match state {
        FieldState::Password => Err(Box::new(Capture::Refused("Cue does not read from password fields.".into()))),
        FieldState::Empty if trusts_empty(&target.class, &foreground_app()) => Err(Box::new(Capture::Empty)),
        FieldState::Empty => Ok(true),
        FieldState::Other => Ok(false),
    }
}

/// Whether the window is a terminal. Input: the window class and the program name (lower case, no `.exe`). Return: whether it is a terminal.
fn is_terminal(class: &str, exe: &str) -> bool {
    const CLASSES: [&str; 4] = ["CASCADIA_HOSTING_WINDOW_CLASS", "ConsoleWindowClass", "mintty", "VirtualConsoleClass"];
    const EXES: [&str; 14] = [
        "windowsterminal",
        "windowsterminalpreview",
        "wt",
        "cmd",
        "conhost",
        "openconsole",
        "powershell",
        "pwsh",
        "wezterm-gui",
        "alacritty",
        "mintty",
        "kitty",
        "putty",
        "ghostty",
    ];
    CLASSES.contains(&class) || EXES.contains(&exe)
}

/// The keys that copy the selection: Ctrl+C everywhere, but Ctrl+Shift+C in a terminal, where Ctrl+C interrupts the running program when nothing is selected and Ctrl+Shift+C copies the selection and does nothing otherwise. Input: the window class and the program name. Return: the key events.
fn copy_keys(class: &str, exe: &str) -> Vec<INPUT> {
    if is_terminal(class, exe) {
        return ctrl_shift_chord(VK_C).to_vec();
    }
    key_chord(VK_CONTROL, VK_C).to_vec()
}

/// Whether a field's "nothing is selected" answer can be believed before Ctrl+C is sent. A browser's text surface (Google Docs, Office on the web) answers with a collapsed range while text is selected, so there the copy is always tried and an unchanged clipboard is what proves nothing was selected; elsewhere the answer stands, which also keeps Ctrl+C from reaching a terminal as an interrupt. Input: the window class and the program name (lower case, no `.exe`). Return: whether the answer stands.
fn trusts_empty(class: &str, exe: &str) -> bool {
    const BROWSERS: [&str; 7] = ["chrome", "msedge", "brave", "vivaldi", "opera", "firefox", "arc"];
    !(matches!(class, "Chrome_WidgetWin_1" | "MozillaWindowClass") && BROWSERS.contains(&exe))
}

/// Sends Ctrl+C with the shortcut's held keys lifted, waits for the clipboard-change message or one deadline, and reads the copy with its owner and the facts that tie it to the target. The clipboard was admitted at sequence `before` with the change listener already registered, so a foreign copy after the admission is heard as an update: one heard before Ctrl+C goes out ends the capture unknown, one heard after makes two updates and the copy is not accepted. Ok(None) when nothing changed in time or the clipboard changed first. Err with a line for the user when the key could not be sent or the clipboard stayed busy.
fn copy_and_look(owner: &Owner, target: &Target, before: u32) -> Result<Option<(Looked, Signs, Target)>, String> {
    owner.drain();
    if owner.updates() > 0 || sequence() != before {
        return Ok(None);
    }
    let Some(bound) = bind(target, &target_now(true)) else {
        crate::log::note("selection: the focus left the window before Ctrl+C, nothing was sent");
        return Ok(None);
    };
    {
        let _lift = Lift::new()?;
        send(&copy_keys(&target.class, &foreground_app()))?;
    }
    owner.pump_until(Instant::now() + COPY_TIMEOUT, || owner.updates() > 0);
    if owner.updates() == 0 {
        return Ok(None);
    }
    let looked = look_within(TEXT_BYTES, HTML_BYTES).ok_or("The clipboard is busy, so Cue could not read the selection.")?;
    owner.drain();
    let now = target_now(true);
    let signs = Signs {
        before,
        after: looked.sequence,
        updates: owner.updates(),
        still_there: now.window != 0 && bound.same_as(&now),
        owner_pid: looked.owner_pid,
        target_pid: target.pid,
        owner_in_tree: looked.owner_pid != 0 && looked.owner_pid != target.pid && descends_from(looked.owner_pid, target.pid),
    };
    Ok(Some((looked, signs, bound)))
}

/// The token delivery will check for a copy about to be made from `now`: the window the shortcut was pressed in (None when focus has left it or is unknown) with the field that has the keyboard at this moment, so the result goes back only to the field it was read from.
fn bind(window: &Target, now: &Target) -> Option<Target> {
    if now.window == 0 || !window.same_as(now) {
        return None;
    }
    Some(Target { runtime_id: now.runtime_id.clone(), key: now.key.clone(), ..window.clone() })
}

/// What the read means. A change that is not provably the target's copy is unknown and the clipboard is left as it is now; the target's own copy is used, and the old clipboard goes back only against that copy's own sequence number.
fn settle(borrow: Borrow, owner: &Owner, looked: Looked, signs: &Signs, bound: Target) -> Capture {
    if !accepted(signs) {
        crate::log::note(&format!("selection: the clipboard changed but not provably by the target, nothing used ({signs:?})"));
        return Capture::Unknown;
    }
    let restored = borrow
        .finish(Ours::Sequence(looked.sequence), owner, None)
        .inspect_err(|why| crate::log::note(&format!("selection: could not put the clipboard back: {why}")))
        .is_ok();
    if looked.too_large {
        return Capture::Refused("The selection is too long.".into());
    }
    match looked.text.filter(|t| !t.is_empty()) {
        Some(text) => Capture::Text(Copied { text, html: looked.html, restored, target: bound }),
        None => Capture::Empty,
    }
}

/// The selection as UI Automation knows it (without its formatting) when the clipboard cannot be borrowed, bound to the element the text was actually read from; otherwise `why` it cannot be read. Unknown when the focus left the window of `target` or moved to another field during the read.
fn uia_fallback(session: Option<&Session>, why: String, target: &Target) -> Capture {
    let Some((found, field)) = session.and_then(|s| selection_in(s, MAX_UIA_CHARS)) else { return Capture::Refused(why) };
    let read = Target { runtime_id: Some(field.runtime_id), key: field.key, ..foreground() };
    match bind(target, &read) {
        Some(bound) => Capture::Text(Copied { text: found.text, html: None, restored: true, target: bound }),
        None => Capture::Unknown,
    }
}

/// Whether the focus is still at the place `target` names, asked right now. An unknown foreground never passes.
fn on_target(target: &Target) -> bool {
    bind(target, &target_now(false)).is_some()
}

#[cfg(test)]
mod tests {
    use super::{bind, is_terminal, trusts_empty};
    use crate::platform::Target;

    /// Terminals are told by window class or program (they copy with Ctrl+Shift+C); editors, browsers and chat apps are not terminals and copy with Ctrl+C.
    #[test]
    fn terminals_are_told_by_class_or_program() {
        assert!(is_terminal("CASCADIA_HOSTING_WINDOW_CLASS", "windowsterminal"));
        assert!(is_terminal("ConsoleWindowClass", "cmd"));
        assert!(is_terminal("Window Class", "alacritty"));
        assert!(is_terminal("mintty", "bash"));
        assert!(!is_terminal("Chrome_WidgetWin_1", "code"));
        assert!(!is_terminal("Chrome_WidgetWin_1", "chrome"));
        assert!(!is_terminal("Notepad", "notepad"));
    }

    /// A browser's empty answer is not believed; other windows (a terminal, a chat app, Notepad) keep theirs.
    #[test]
    fn only_a_browser_surface_is_copied_after_an_empty_answer() {
        assert!(!trusts_empty("Chrome_WidgetWin_1", "chrome"));
        assert!(!trusts_empty("Chrome_WidgetWin_1", "msedge"));
        assert!(!trusts_empty("MozillaWindowClass", "firefox"));
        assert!(trusts_empty("Chrome_WidgetWin_1", "code"));
        assert!(trusts_empty("CASCADIA_HOSTING_WINDOW_CLASS", "windowsterminal"));
        assert!(trusts_empty("Notepad", "notepad"));
    }

    /// The copy is bound to the field that has the keyboard when Ctrl+C goes out, in the same window; another window or an unknown foreground binds nothing.
    #[test]
    fn a_copy_is_bound_to_the_field_in_focus_at_the_send() {
        let window = Target { window: 7, pid: 1, thread: 2, ..Target::default() };
        let now = Target { runtime_id: Some(vec![3, 4]), key: "50004:b".into(), ..window.clone() };
        let bound = bind(&window, &now).unwrap();
        assert_eq!((bound.runtime_id, bound.key), (Some(vec![3, 4]), "50004:b".to_string()));
        assert!(bind(&window, &Target { window: 8, ..now.clone() }).is_none());
        assert!(bind(&window, &Target::default()).is_none());
    }
}
