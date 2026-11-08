//! File: mod.rs
//! Purpose: The tray menu as an app-owned popup window, the way Pouchy does it (a styled popup the app builds on each
//! right click, placed at the mouse point, dismissed by an outside click), but as a plain Win32 layered window drawn in
//! software, so nothing stays resident and there is no system menu frame. It never takes focus, so the tray overflow
//! panel it was opened from stays open.
//! Contents: show_menu — open one menu and return the chosen id (None: cancelled; Err: it could not open); wndproc — pointer, keys and display changes;
//! place — position at the mouse point, flipped to fit the screen. Drawing is in `draw`, input hooks in `hooks`, the menu's resources in `session`.

mod draw;
mod hooks;
mod icons;
mod session;

use std::cell::{Cell, RefCell};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DOWN, VK_ESCAPE, VK_RETURN, VK_UP};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::platform::{MenuEntry, Point, Rect, Theme};
use draw::{Layout, Palette, Surface};
use hooks::KEY_MESSAGE;
use session::Session;

/// dip the menu sits above the mouse point when it opens upward, so it clears the tray icon.
const RAISE: f32 = 14.0;

/// What the open menu draws from. Lives on the menu thread only.
struct State {
    entries: Vec<MenuEntry>,
    layout: Layout,
    palette: Palette,
    surface: Surface,
    hover: Option<usize>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static CHOICE: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Top-left at the mouse point like a WPF `ContextMenu`; where that would run off the screen the menu flips to the other
/// side of the point (above it, or to its left). Above the point it sits `RAISE` higher so it clears the tray icon.
fn place(at: POINT, (w, h): (i32, i32), work: &Rect, scale: f32) -> (i32, i32) {
    let x = if at.x + w > work.right { at.x - w } else { at.x };
    let y = if at.y + h > work.bottom { at.y - h - draw::px(RAISE, scale) } else { at.y };
    (x.clamp(work.left, (work.right - w).max(work.left)), y.clamp(work.top, (work.bottom - h).max(work.top)))
}

/// Pushes the surface to the layered window, moving it to `at` when given. Safety: `hwnd` is the menu window and the surface DC is live, both used on the menu thread.
fn present(hwnd: HWND, s: &Surface, at: Option<POINT>) {
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let size = SIZE { cx: s.w, cy: s.h };
    let _ = unsafe {
        UpdateLayeredWindow(
            hwnd,
            None,
            at.as_ref().map(|p| p as *const POINT),
            Some(&size),
            Some(s.dc),
            Some(&POINT::default()),
            Default::default(),
            Some(&blend),
            ULW_ALPHA,
        )
    };
}

/// Redraws the menu from its state and presents it.
fn repaint(hwnd: HWND) {
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            draw::paint(&mut st.surface, &st.entries, &st.layout, &st.palette, st.hover);
            present(hwnd, &st.surface, None);
        }
    });
}

/// Moves the highlight to row `hover` and repaints when it changed.
fn set_hover(hwnd: HWND, hover: Option<usize>) {
    let changed = STATE.with(|s| s.borrow_mut().as_mut().is_some_and(|st| std::mem::replace(&mut st.hover, hover) != hover));
    if changed {
        repaint(hwnd);
    }
}

/// Records the id of row `index` (None for a separator or no row) as the choice and closes the menu. Safety: destroys the menu window of this thread.
fn choose(hwnd: HWND, index: Option<usize>) {
    let id = STATE.with(|s| {
        index.and_then(|i| match s.borrow().as_ref()?.entries.get(i)? {
            MenuEntry::Item { id, .. } => Some(*id),
            MenuEntry::Separator => None,
        })
    });
    CHOICE.set(id);
    unsafe { drop(DestroyWindow(hwnd)) };
}

/// Escape closes, Enter chooses the highlighted row, Up and Down move the highlight. Safety: destroys the menu window of this thread.
fn on_key(hwnd: HWND, key: u16) {
    let (hover, next) = STATE
        .with(|s| {
            let state = s.borrow();
            let st = state.as_ref()?;
            let step = if key == VK_DOWN.0 { 1 } else { -1 };
            Some((st.hover, draw::next_item(&st.entries, st.hover, step)))
        })
        .unwrap_or_default();
    match key {
        k if k == VK_ESCAPE.0 => unsafe { drop(DestroyWindow(hwnd)) },
        k if k == VK_RETURN.0 => choose(hwnd, hover),
        k if k == VK_DOWN.0 || k == VK_UP.0 => set_hover(hwnd, next),
        _ => {}
    }
}

/// The row under the pointer for a mouse message's `lp`.
fn pointer_row(lp: LPARAM) -> Option<usize> {
    let y = (lp.0 >> 16 & 0xFFFF) as i16 as i32;
    STATE.with(|s| s.borrow().as_ref().and_then(|st| draw::hit(&st.layout, &st.entries, y)))
}

/// A panic must not unwind out of the window procedure (it would abort the app), so it is contained and the message is left unhandled.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    crate::guard::contain(|| handle(hwnd, msg, wp, lp)).unwrap_or(LRESULT(0))
}

/// Handles one menu message. Input: the window and the message. Return: 0 when handled, else the default procedure's answer. Never activate: taking focus would close the tray overflow panel behind the menu.
unsafe fn handle(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => set_hover(hwnd, pointer_row(lp)),
        WM_LBUTTONUP => choose(hwnd, pointer_row(lp)),
        KEY_MESSAGE => on_key(hwnd, wp.0 as u16),
        WM_CLOSE | WM_DISPLAYCHANGE | WM_DPICHANGED => drop(DestroyWindow(hwnd)),
        WM_SETTINGCHANGE if wp.0 == SPI_SETWORKAREA.0 as usize => drop(DestroyWindow(hwnd)),
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_DESTROY => PostQuitMessage(0),
        WM_ERASEBKGND => return LRESULT(1),
        _ => return DefWindowProcW(hwnd, msg, wp, lp),
    }
    LRESULT(0)
}

static REGISTER: std::sync::Once = std::sync::Once::new();

/// Makes the layered popup window at `(x, y)`. Return: None when Windows refuses.
fn create_window(x: i32, y: i32, size: (i32, i32)) -> Option<HWND> {
    unsafe {
        let module = GetModuleHandleW(None).ok()?;
        let class = w!("CueTrayMenu");
        let cursor = LoadCursorW(None, IDC_ARROW).ok()?;
        REGISTER.call_once(|| {
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: module.into(),
                lpszClassName: class,
                hCursor: cursor,
                ..Default::default()
            });
        });
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class,
            w!("Cue Menu"),
            WS_POPUP,
            x,
            y,
            size.0,
            size.1,
            None,
            None,
            Some(module.into()),
            None,
        )
        .ok()
    }
}

/// Opens one menu and waits for it to close. Call from a worker thread, never the tray's own handler. Input: the rows, the mouse point and the user's theme. Return: `Ok(Some(id))` for a chosen item, `Ok(None)` when the user dismissed it, `Err` with a sentence when it could not open.
pub fn show_menu(entries: Vec<MenuEntry>, at: Point, theme: &Theme) -> Result<Option<u32>, String> {
    Ok(Session::open(entries, at, theme)?.run())
}
