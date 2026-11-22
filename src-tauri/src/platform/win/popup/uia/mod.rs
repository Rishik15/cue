//! File: uia/mod.rs
//! Purpose: Lets a screen reader read the tray menu. The menu is one painted window, so without this Narrator sees an unnamed picture. On `WM_GETOBJECT` (Microsoft: "Handling the WM_GETOBJECT Message", `UiaReturnRawElementProvider`) the window answers with a UI Automation fragment: a Menu root whose children are MenuItems with a name, position in set and size of set, a rectangle, the Invoke pattern, and a focus-changed event when the highlighted row moves. Nothing is built until a client asks, so an idle menu costs nothing. A client may keep a provider after the menu closes, so every provider checks a shared "open" flag, and every request posted back to the menu carries the menu's token so a later menu (or a window handle Windows reused) ignores it.
//! Contents: Shared — the rows, focus and open flag, read from any thread; Shared::new — state seeded with the current highlight; provider_for — the answer to `WM_GETOBJECT`; focus_moved — announce the highlighted row; release — close the menu's providers and tell UI Automation the window is gone; is_current — whether a posted request belongs to the open menu; INVOKE_MESSAGE / FOCUS_MESSAGE — what an item posts back. Submodules: ifaces (the provider interfaces, NULL-capable), props (shared constants and helpers), root and item (the providers).

mod ifaces;
mod item;
mod props;
mod root;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::sync::Arc;

use windows::core::{Error, Interface, Result, HRESULT};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::WM_APP;

use super::rows::Row;

/// Posted to the menu window when a client invokes an item: `wparam` is the item's entry index, `lparam` the menu's token.
pub const INVOKE_MESSAGE: u32 = WM_APP + 2;
/// Posted when a client moves focus to an item: `wparam` is the item's entry index, `lparam` the menu's token.
pub const FOCUS_MESSAGE: u32 = WM_APP + 3;

/// What the providers read. Built once on the menu thread and then only read, except `focus` and `open`, so UI Automation may call from any thread. `focus` is the highlighted row's position in `rows`, or -1. `open` turns false when the menu closes. `token` tells this menu's requests from any other's.
struct Shared {
    hwnd: isize,
    token: usize,
    rows: Vec<Row>,
    focus: AtomicIsize,
    open: AtomicBool,
}

thread_local! {
    static PROVIDER: RefCell<Option<Arc<Shared>>> = const { RefCell::new(None) };
}

static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(1);

impl Shared {
    /// A menu's shared state, open. Input: the window, its rows and the entry highlighted right now (a client may attach after a row was highlighted). Return: the state with a token no earlier menu had.
    fn new(hwnd: HWND, rows: Vec<Row>, hover: Option<usize>) -> Self {
        let focus = AtomicIsize::new(position(&rows, hover).map_or(-1, |p| p as isize));
        Shared { hwnd: hwnd.0 as isize, token: NEXT_TOKEN.fetch_add(1, Ordering::Relaxed), rows, focus, open: AtomicBool::new(true) }
    }

    /// The menu window. Return: its handle.
    fn window(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    /// Whether the menu is still open. Return: Ok, or `UIA_E_ELEMENTNOTAVAILABLE`, the answer for an element that is gone.
    fn live(&self) -> Result<()> {
        if self.open.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(Error::from(HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32)))
        }
    }
}

/// A row's position in `rows`. Input: the rows and an entry index. Return: the position, or None.
fn position(rows: &[Row], entry: Option<usize>) -> Option<usize> {
    entry.and_then(|e| rows.iter().position(|r| r.entry == e))
}

/// The state of the open menu, made on the first ask. Input: the window and how to read the rows and the highlighted entry. Return: the shared state.
fn session(hwnd: HWND, state: impl FnOnce() -> (Vec<Row>, Option<usize>)) -> Arc<Shared> {
    PROVIDER.with(|p| {
        p.borrow_mut()
            .get_or_insert_with(|| {
                let (rows, hover) = state();
                Arc::new(Shared::new(hwnd, rows, hover))
            })
            .clone()
    })
}

/// The answer to `WM_GETOBJECT`. The message is passed on unfiltered, as Microsoft requires (UI Automation and Active Accessibility clients are told apart inside the call). The provider is built here, on the first ask. Input: the window, the message parameters, and how to read the menu's rows and highlighted entry. Return: the message result to return from the window procedure. Safety: `UiaReturnRawElementProvider` is given the message parameters unfiltered, as Microsoft requires.
pub fn provider_for(hwnd: HWND, wp: WPARAM, lp: LPARAM, state: impl FnOnce() -> (Vec<Row>, Option<usize>)) -> LRESULT {
    let Ok(root) = session(hwnd, state).root().cast::<IRawElementProviderSimple>() else { return LRESULT(0) };
    unsafe { UiaReturnRawElementProvider(hwnd, wp, lp, &root) }
}

/// Announces the newly highlighted row (a focus-changed event) when a client is listening and a provider was ever asked for. Input: the entry index now highlighted, or None. Costs one thread-local read otherwise. Safety: raises an event for a provider this thread made.
pub fn focus_moved(entry: Option<usize>) {
    let Some(shared) = PROVIDER.with(|p| p.borrow().clone()) else { return };
    let pos = position(&shared.rows, entry);
    shared.focus.store(pos.map_or(-1, |p| p as isize), Ordering::Release);
    if let Some(pos) = pos {
        if unsafe { UiaClientsAreListening() }.as_bool() {
            if let Ok(item) = shared.item(pos).cast::<IRawElementProviderSimple>() {
                let _ = unsafe { UiaRaiseAutomationEvent(&item, UIA_AutomationFocusChangedEventId) };
            }
        }
    }
}

/// Whether a posted request belongs to the open menu. Input: the request's `lparam`. Return: true only for the token of the menu whose providers are live.
pub fn is_current(lp: LPARAM) -> bool {
    PROVIDER.with(|p| p.borrow().as_ref().is_some_and(|s| s.token as isize == lp.0))
}

/// Closes the menu's providers and forgets them: clients that kept one now get `UIA_E_ELEMENTNOTAVAILABLE`. Return: whether a provider had been made.
fn retire() -> bool {
    PROVIDER.with(|p| p.borrow_mut().take()).inspect(|s| s.open.store(false, Ordering::Release)).is_some()
}

/// Called from `WM_DESTROY`: closes the providers, then tells UI Automation the window is gone so it drops what it kept for it (Microsoft's recommended call). It does not call `UiaDisconnectProvider`: Microsoft forbids that inside a synchronously sent message, and the open flag already makes every retained provider answer "unavailable". Safety: the documented `UiaReturnRawElementProvider(hwnd, 0, 0, NULL)` call.
pub fn release(hwnd: HWND) {
    if retire() {
        unsafe { UiaReturnRawElementProvider(hwnd, WPARAM(0), LPARAM(0), None::<&IRawElementProviderSimple>) };
    }
}
