//! Purpose: The tray menu as an app-owned popup window, the way Pouchy does it (a styled popup the app builds on each
//! right click, placed at the mouse point, dismissed by an outside click), but as a plain Win32 layered window drawn in
//! software, so nothing stays resident and there is no system menu frame. It never takes focus, so the tray overflow
//! panel it was opened from stays open.
//! Contents: show_menu — build, run and tear down one menu, returning the chosen id; wndproc — pointer and keys;
//! place — position at the mouse point, flipped to fit the screen. Drawing is in `draw`, input hooks in `hooks`.

mod draw;
mod hooks;
mod icons;

use std::cell::{Cell, RefCell};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DOWN, VK_ESCAPE, VK_RETURN, VK_UP};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::theme::system_uses_dark_theme;
use crate::platform::{MenuEntry, Point};
use draw::{Layout, Palette, Surface};
use hooks::{Hooks, KEY_MESSAGE};

/// dip the menu sits above the mouse point when it opens upward, so it clears the tray icon.
const RAISE: f32 = 14.0;

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

fn monitor_scale_and_work_area(at: POINT) -> (f32, RECT) {
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    let (mut dpi_x, mut dpi_y) = (96, 96);
    unsafe {
        let monitor = MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(monitor, &mut info);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    }
    (dpi_x.max(96) as f32 / 96.0, info.rcWork)
}

/// Top-left at the mouse point like a WPF `ContextMenu`; where that would run off the screen the menu flips to the other
/// side of the point (above it, or to its left). Above the point it sits `RAISE` higher so it clears the tray icon.
fn place(at: POINT, (w, h): (i32, i32), work: &RECT, scale: f32) -> (i32, i32) {
    let x = if at.x + w > work.right { at.x - w } else { at.x };
    let y = if at.y + h > work.bottom { at.y - h - draw::px(RAISE, scale) } else { at.y };
    (x.clamp(work.left, (work.right - w).max(work.left)), y.clamp(work.top, (work.bottom - h).max(work.top)))
}

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

fn create_font(scale: f32) -> HFONT {
    let height = -((draw::FONT * scale).round() as i32);
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI Variable Text"),
        )
    }
}

fn repaint(hwnd: HWND) {
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            draw::paint(&mut st.surface, &st.entries, &st.layout, &st.palette, st.hover);
            present(hwnd, &st.surface, None);
        }
    });
}

fn set_hover(hwnd: HWND, hover: Option<usize>) {
    let changed = STATE.with(|s| s.borrow_mut().as_mut().is_some_and(|st| std::mem::replace(&mut st.hover, hover) != hover));
    if changed {
        repaint(hwnd);
    }
}

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

fn on_key(hwnd: HWND, key: u16) {
    let (hover, next) = STATE
        .with(|s| {
            let state = s.borrow();
            let st = state.as_ref()?;
            let step = if key == VK_DOWN.0 { 1 } else { -1 };
            Some((st.hover, draw::next_enabled(&st.entries, st.hover, step)))
        })
        .unwrap_or_default();
    match key {
        k if k == VK_ESCAPE.0 => unsafe { drop(DestroyWindow(hwnd)) },
        k if k == VK_RETURN.0 => choose(hwnd, hover),
        k if k == VK_DOWN.0 || k == VK_UP.0 => set_hover(hwnd, next),
        _ => {}
    }
}

fn pointer_row(lp: LPARAM) -> Option<usize> {
    let y = (lp.0 >> 16 & 0xFFFF) as i16 as i32;
    STATE.with(|s| s.borrow().as_ref().and_then(|st| draw::hit(&st.layout, &st.entries, y)))
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => set_hover(hwnd, pointer_row(lp)),
        WM_LBUTTONUP => choose(hwnd, pointer_row(lp)),
        KEY_MESSAGE => on_key(hwnd, wp.0 as u16),
        WM_CLOSE => drop(DestroyWindow(hwnd)),
        // Never activate: taking focus would close the tray overflow panel behind the menu.
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_DESTROY => PostQuitMessage(0),
        WM_ERASEBKGND => return LRESULT(1),
        _ => return DefWindowProcW(hwnd, msg, wp, lp),
    }
    LRESULT(0)
}

fn create_window(x: i32, y: i32, size: (i32, i32)) -> Option<HWND> {
    unsafe {
        let module = GetModuleHandleW(None).ok()?;
        let class = w!("CueTrayMenu");
        let cursor = LoadCursorW(None, IDC_ARROW).ok()?;
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: module.into(),
            lpszClassName: class,
            hCursor: cursor,
            ..Default::default()
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

fn run_loop() {
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Blocks until the menu closes; returns the chosen item's id. Call from a worker thread, never the tray's own handler.
pub fn show_menu(entries: Vec<MenuEntry>, at: Point) -> Option<u32> {
    let cursor = POINT { x: at.x, y: at.y };
    let (scale, work) = monitor_scale_and_work_area(cursor);
    let font = create_font(scale);
    let mut surface = None;
    let layout = unsafe {
        let measure = CreateCompatibleDC(None);
        let old = SelectObject(measure, font.into());
        let layout = draw::layout(measure, &entries, scale);
        SelectObject(measure, old);
        let _ = DeleteDC(measure);
        layout
    };
    let (x, y) = place(cursor, (layout.width, layout.height), &work, scale);
    if let Some(s) = Surface::new(layout.width, layout.height) {
        unsafe { SelectObject(s.dc, font.into()) };
        surface = Some(s);
    }
    let result = surface.and_then(|surface| {
        let hwnd = create_window(x, y, (layout.width, layout.height))?;
        STATE.with(|s| {
            *s.borrow_mut() = Some(State { entries, layout, palette: Palette::new(system_uses_dark_theme()), surface, hover: None })
        });
        CHOICE.set(None);
        repaint(hwnd);
        let _ = STATE.with(|s| s.borrow().as_ref().map(|st| present(hwnd, &st.surface, Some(POINT { x, y }))));
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        let hooks = Hooks::install(hwnd);
        run_loop();
        drop(hooks);
        Some(CHOICE.take())
    });
    STATE.with(|s| s.borrow_mut().take());
    let _ = unsafe { DeleteObject(font.into()) };
    result.flatten()
}
