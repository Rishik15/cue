//! Purpose: Reports what the user types, for snippet expansion (Windows): keyboard Raw Input received by a hidden message-only window on its own thread, which exists only while expansion is on, so idle Cue has neither.
//! Raw Input is Microsoft's recommended way to watch keys that are not blocked: it is asynchronous, has no time limit and, unlike a low-level hook, cannot be removed by Windows when the app is slow.
//! Contents: watch — start the window and thread (or swap the handler); stop — end them; window_proc — receives WM_INPUT; on_raw — one raw key event; note_key / translate / character — one key press to a `Typed`
//! (a character in the keyboard layout in use, Backspace, or Reset for anything that breaks a word: arrows, shortcuts, a change of window or a long pause). Keys Cue injected itself (no device behind them) are ignored,
//! so an expansion never triggers another. Nothing typed is stored or logged here.

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::sync::atomic::{AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK, RIDEV_REMOVE,
    RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::platform::Typed;

type Handler = Arc<dyn Fn(Typed) + Send + Sync>;

static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);
/// The watcher thread's id while it runs (0 when it does not), for `stop`.
static THREAD: AtomicU32 = AtomicU32::new(0);
static LAST_WINDOW: AtomicIsize = AtomicIsize::new(0);
static LAST_TICK: AtomicU64 = AtomicU64::new(0);
/// A pause this long ends the word: the user went away and came back.
const PAUSE_MS: u64 = 4_000;
/// `RI_KEY_BREAK`: the event is a key coming up.
const KEY_UP: u16 = 1;

pub fn watch(handler: Box<dyn Fn(Typed) + Send + Sync>) -> Result<(), String> {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(handler.into());
    if THREAD.load(Ordering::Acquire) != 0 {
        return Ok(());
    }
    let (ready, started) = mpsc::channel();
    crate::guard::spawn("typing", 256 * 1024, move || run(ready)).map_err(|e| format!("Could not start watching the keyboard: {e}"))?;
    started.recv_timeout(Duration::from_secs(2)).map_err(|_| "The keyboard watcher did not start.".to_string())?
}

pub fn stop() {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let thread = THREAD.load(Ordering::Acquire);
    if thread != 0 {
        // SAFETY: wakes the watcher's message loop so it ends and removes its window.
        let _ = unsafe { PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0)) };
        // A watcher started right after must find the old one gone, not winding down.
        for _ in 0..100 {
            if THREAD.load(Ordering::Acquire) == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn start_window() -> Result<HWND, String> {
    // SAFETY: a window class and a message-only window owned by this thread, destroyed by `run`.
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
        RegisterRawInputDevices(&[keyboard], size_of::<RAWINPUTDEVICE>() as u32).map_err(|e| e.to_string())?;
        Ok(window)
    }
}

fn run(ready: mpsc::Sender<Result<(), String>>) {
    let window = match start_window() {
        Ok(window) => window,
        Err(e) => return drop(ready.send(Err(format!("Could not watch the keyboard: {e}")))),
    };
    // SAFETY: pumps the messages of the window made above on this thread, then removes the registration and the window.
    unsafe {
        THREAD.store(GetCurrentThreadId(), Ordering::Release);
        let _ = ready.send(Ok(()));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = DispatchMessageW(&msg);
        }
        let remove = RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: RIDEV_REMOVE, hwndTarget: HWND::default() };
        let _ = RegisterRawInputDevices(&[remove], size_of::<RAWINPUTDEVICE>() as u32);
        let _ = DestroyWindow(window);
        THREAD.store(0, Ordering::Release);
    }
}

fn emit(event: Typed) {
    let handler = HANDLER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(handler) = handler {
        handler(event);
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_INPUT {
        on_raw(HRAWINPUT(lp.0 as *mut c_void));
    }
    DefWindowProcW(hwnd, msg, wp, lp) // Windows needs this call for WM_INPUT to release the event
}

/// One raw event: a key going down that came from a keyboard (a key Cue sent has no device) is passed on.
fn on_raw(input: HRAWINPUT) {
    // SAFETY: the event's data is copied into a buffer of the size of one RAWINPUT; a key event fits.
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
    // SAFETY: the type says which member of the union holds the data.
    let key = unsafe { raw.data.keyboard };
    if raw.header.dwType != RIM_TYPEKEYBOARD.0 || key.Flags & KEY_UP != 0 || raw.header.hDevice.is_invalid() || key.VKey == 0xFF {
        return;
    }
    note_key(u32::from(key.VKey), u32::from(key.MakeCode));
}

/// Sends what the key means. Short on purpose: it runs between the user's keystrokes.
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

fn held(vk: VIRTUAL_KEY) -> bool {
    (unsafe { GetAsyncKeyState(vk.0 as i32) }) as u16 & 0x8000 != 0
}

fn translate(vk: u32, scan: u32) -> Option<Typed> {
    let key = VIRTUAL_KEY(vk as u16);
    match key {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN
        | VK_CAPITAL | VK_NUMLOCK | VK_SCROLL => None,
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

/// The character the key types with the modifiers now held. Ctrl or Alt on their own make a shortcut; both together are AltGr, which types.
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
    // SAFETY: the layout of the focused window's thread; flag 4 leaves the keyboard's dead-key state alone.
    let n = unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        ToUnicodeEx(key.0 as u32, scan, &state, &mut out, 0x4, Some(GetKeyboardLayout(thread)))
    };
    let c = char::decode_utf16(out.iter().copied().take(n.max(0) as usize)).next()?.ok()?;
    Some(if c.is_control() { Typed::Reset } else { Typed::Char(c) })
}
