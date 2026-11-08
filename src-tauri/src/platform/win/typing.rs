//! File: typing.rs
//! Purpose: Reports what the user types, for snippet expansion (Windows): keyboard Raw Input received by a hidden message-only window on its own thread, which exists only while expansion is on, so idle Cue has neither.
//! Raw Input is Microsoft's recommended way to watch keys that are not blocked: it is asynchronous, has no time limit and, unlike a low-level hook, cannot be removed by Windows when the app is slow.
//! Contents: watch — start the window and thread (or swap the handler), through `watcher::Lifecycle`; stop — end them; start_window / run — the window and its message loop; emit — pass an event to the handler; window_proc — receives WM_INPUT; on_raw — one raw key event; note_key / held / translate / character — one key press to a `Typed`
//! (a character in the keyboard layout in use, Backspace, or Reset for anything that breaks a word: arrows, shortcuts, a change of window or a long pause). Keys Cue injected itself (no device behind them) are ignored,
//! so an expansion never triggers another. Nothing typed is stored or logged here.

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK, RIDEV_REMOVE,
    RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::watcher::{Lifecycle, Ready};
use crate::platform::Typed;

/// What is called with each typed event.
type Handler = Arc<dyn Fn(Typed) + Send + Sync>;

/// The current handler; None while expansion is off.
static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);
/// The thread and window lifecycle.
static WATCHER: Lifecycle = Lifecycle::new();
/// The foreground window at the last key, to tell a change of window.
static LAST_WINDOW: AtomicIsize = AtomicIsize::new(0);
/// The tick count at the last key, to tell a pause.
static LAST_TICK: AtomicU64 = AtomicU64::new(0);
/// A pause this long ends the word: the user went away and came back.
const PAUSE_MS: u64 = 4_000;
/// `RI_KEY_BREAK`: the event is a key coming up.
const KEY_UP: u16 = 1;

/// Starts watching the keyboard (or swaps the handler of a watch already running); `handler` gets each `Typed` event. Err with a line for the user when the window cannot be made.
pub fn watch(handler: Box<dyn Fn(Typed) + Send + Sync>) -> Result<(), String> {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(handler.into());
    WATCHER.start("typing", 256 * 1024, "keyboard", run).inspect_err(|_| *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None)
}

/// Ends the watch and drops the handler.
pub fn stop() {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    WATCHER.stop();
}

/// Makes the message-only window and registers it for keyboard Raw Input. Safety: a window class and a message-only window owned by this thread, destroyed by `run`.
fn start_window() -> Result<HWND, String> {
    unsafe {
        let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("CueTypingWatcher");
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: module.into(), lpszClassName: class, ..Default::default() });
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(module.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        let keyboard = RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: RIDEV_INPUTSINK, hwndTarget: window };
        if let Err(e) = RegisterRawInputDevices(&[keyboard], size_of::<RAWINPUTDEVICE>() as u32) {
            let _ = DestroyWindow(window);
            return Err(e.to_string());
        }
        Ok(window)
    }
}

/// The watcher thread: makes the window, pumps its messages until stopped, then removes the registration and the window. Safety: only the window made here is used, on this thread.
fn run(ready: Ready) {
    let window = match start_window() {
        Ok(window) => window,
        Err(e) => return ready.failed(format!("Could not watch the keyboard: {e}")),
    };
    unsafe {
        if ready.running() {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = DispatchMessageW(&msg);
            }
        }
        let remove = RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: RIDEV_REMOVE, hwndTarget: HWND::default() };
        let _ = RegisterRawInputDevices(&[remove], size_of::<RAWINPUTDEVICE>() as u32);
        let _ = DestroyWindow(window);
    }
    ready.ended();
}

/// Passes the event to the current handler, if any.
fn emit(event: Typed) {
    let handler = HANDLER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(handler) = handler {
        handler(event);
    }
}

/// Window procedure: `WM_INPUT` is handled under `guard::contain`, because a panic must not unwind through Windows; the default procedure is always called, as Windows needs that for `WM_INPUT` to release the event.
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_INPUT {
        let _ = crate::guard::contain(|| on_raw(HRAWINPUT(lp.0 as *mut c_void)));
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

/// One raw event: a key going down that came from a keyboard (a key Cue sent has no device) is passed on. Safety: the event's data is copied into a buffer of the size of one RAWINPUT, which a key event fits; the type says which member of the union holds the data.
fn on_raw(input: HRAWINPUT) {
    let raw: RAWINPUT = unsafe {
        let mut raw: RAWINPUT = zeroed();
        let mut size = size_of::<RAWINPUT>() as u32;
        if GetRawInputData(input, RID_INPUT, Some(&mut raw as *mut RAWINPUT as *mut c_void), &mut size, size_of::<RAWINPUTHEADER>() as u32)
            == u32::MAX
        {
            return;
        }
        raw
    };
    let key = unsafe { raw.data.keyboard };
    if raw.header.dwType != RIM_TYPEKEYBOARD.0 || key.Flags & KEY_UP != 0 || raw.header.hDevice.is_invalid() || key.VKey == 0xFF {
        return;
    }
    note_key(u32::from(key.VKey), u32::from(key.MakeCode));
}

/// Sends what the key means. Short on purpose: it runs between the user's keystrokes. Safety: `GetForegroundWindow` and `GetTickCount64` take no arguments; the window handle is only stored as an integer, never dereferenced.
fn note_key(vk: u32, scan: u32) {
    let (window, now) = (unsafe { GetForegroundWindow() }.0 as isize, unsafe { GetTickCount64() });
    let moved = LAST_WINDOW.swap(window, Ordering::AcqRel) != window;
    let paused = now.saturating_sub(LAST_TICK.swap(now, Ordering::AcqRel)) > PAUSE_MS;
    if moved || paused {
        emit(Typed::Reset);
    }
    if let Some(event) = translate(vk, scan) {
        emit(event);
    }
}

/// Whether the key is physically down right now. Safety: `GetAsyncKeyState` only reads the key state for a virtual-key code and has no pointer arguments.
fn held(vk: VIRTUAL_KEY) -> bool {
    (unsafe { GetAsyncKeyState(vk.0 as i32) }) as u16 & 0x8000 != 0
}

/// What a key press means: None for a bare modifier, Backspace, Reset for anything that breaks a word (Ctrl+Backspace deletes a word, which the tracker cannot follow), a character otherwise.
fn translate(vk: u32, scan: u32) -> Option<Typed> {
    let key = VIRTUAL_KEY(vk as u16);
    match key {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN
        | VK_CAPITAL | VK_NUMLOCK | VK_SCROLL => None,
        VK_BACK if held(VK_CONTROL) || held(VK_MENU) || held(VK_LWIN) || held(VK_RWIN) => Some(Typed::Reset),
        VK_BACK => Some(Typed::Backspace),
        VK_RETURN => Some(Typed::Char('\n')),
        VK_TAB => Some(Typed::Char('\t')),
        VK_SPACE => Some(Typed::Char(' ')),
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_DELETE | VK_INSERT | VK_ESCAPE => {
            Some(Typed::Reset)
        }
        k if (VK_F1.0..=VK_F24.0).contains(&k.0) => Some(Typed::Reset),
        _ => character(key, scan),
    }
}

/// The character the key types with the modifiers now held. Ctrl or Alt on their own make a shortcut; both together are AltGr, which types. Safety: asks the layout of the focused window's thread; flag 4 leaves the keyboard's dead-key state alone.
fn character(key: VIRTUAL_KEY, scan: u32) -> Option<Typed> {
    let (ctrl, alt) = (held(VK_CONTROL), held(VK_MENU));
    if held(VK_LWIN) || held(VK_RWIN) || ctrl != alt {
        return Some(Typed::Reset);
    }
    let mut state = [0u8; 256];
    for (modifier, down) in [(VK_SHIFT, held(VK_SHIFT)), (VK_CONTROL, ctrl), (VK_MENU, alt)] {
        state[modifier.0 as usize] = if down { 0x80 } else { 0 };
    }
    state[VK_CAPITAL.0 as usize] = (unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1) as u8;
    let mut out = [0u16; 4];
    let n = unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        ToUnicodeEx(key.0 as u32, scan, &state, &mut out, 0x4, Some(GetKeyboardLayout(thread)))
    };
    let c = char::decode_utf16(out.iter().copied().take(n.max(0) as usize)).next()?.ok()?;
    Some(if c.is_control() { Typed::Reset } else { Typed::Char(c) })
}
