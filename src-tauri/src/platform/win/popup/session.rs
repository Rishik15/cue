//! File: session.rs
//! Purpose: One open tray menu as an owner of everything it creates: the font, the window and the input hooks. Building it can fail at any step and dropping it releases whatever exists, so no exit path (cancel, choice, failure, panic unwind) leaks a window or a hook.
//! Contents: Session — `open` builds the menu and shows it, `run` pumps messages until it closes and returns the choice, Drop releases the window, hooks, drawing state and font.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::super::fonts::Private;
use super::super::monitor::at_point;
use super::super::palette::resolve;
use super::super::theme::system_uses_dark_theme;
use super::draw::{self, Palette, Surface};
use super::hooks::Hooks;
use super::{create_window, place, present, repaint, State, CHOICE, STATE};
use crate::platform::{MenuEntry, Point, Theme};

/// An open menu. Fields are filled as steps succeed; `Drop` releases what is there.
pub struct Session {
    _inter: Private,
    font: HFONT,
    hwnd: Option<HWND>,
    hooks: Option<Hooks>,
}

impl Session {
    /// Builds and shows the menu at `at`. Input: the rows, the mouse point in physical pixels and the user's theme. Return: the open session, or a sentence saying which step failed (everything made so far is released). Safety: Shows the window this session just made. Safety: The surface DC is live and owned by the surface; the font outlives it (the session deletes both). Safety: GDI calls on a measuring DC created and deleted here, with the font selected only while measuring. Safety: Creates a GDI font from the loaded Inter files; `Session` deletes it on drop.
    pub fn open(entries: Vec<MenuEntry>, at: Point, theme: &Theme) -> Result<Session, String> {
        let cursor = POINT { x: at.x, y: at.y };
        let display = at_point(cursor.x, cursor.y).ok_or("Windows could not say which screen the menu belongs on")?;
        let inter = Private::load();
        let font = unsafe { inter.create(draw::FONT, display.scale, 400) };
        let mut session = Session { _inter: inter, font, hwnd: None, hooks: None };
        let layout = unsafe {
            let measure = CreateCompatibleDC(None);
            let old = SelectObject(measure, font.into());
            let layout = draw::layout(measure, &entries, display.scale);
            SelectObject(measure, old);
            let _ = DeleteDC(measure);
            layout
        };
        let (x, y) = place(cursor, (layout.width, layout.height), &display.work, display.scale);
        let surface = Surface::new(layout.width, layout.height).ok_or("could not create the menu bitmap")?;
        unsafe { SelectObject(surface.dc, font.into()) };
        let hwnd = create_window(x, y, (layout.width, layout.height)).ok_or("could not create the menu window")?;
        session.hwnd = Some(hwnd);
        STATE.with(|s| {
            *s.borrow_mut() =
                Some(State { entries, layout, palette: Palette::new(&resolve(theme, system_uses_dark_theme())), surface, hover: None });
        });
        CHOICE.set(None);
        repaint(hwnd);
        STATE.with(|s| s.borrow().as_ref().map(|st| present(hwnd, &st.surface, Some(POINT { x, y }))));
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        session.hooks = Some(Hooks::install(hwnd).ok_or("could not install the hooks that close the menu")?);
        Ok(session)
    }

    /// Pumps messages until the menu window is destroyed. Return: the chosen item's id, or None when the menu was cancelled (Escape, an outside click, a window-switching key or a display change). Safety: The standard message loop of the thread that owns the menu window.
    pub fn run(self) -> Option<u32> {
        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        CHOICE.take()
    }
}

impl Drop for Session {
    /// Removes the hooks first (nothing may post to a dying window), then the window, the drawing state and the font. Safety: the window belongs to this thread (destroying one already gone just fails), and the font was created by this session and is selected by nothing once the surface is gone.
    fn drop(&mut self) {
        self.hooks.take();
        if let Some(hwnd) = self.hwnd.take() {
            let _ = unsafe { DestroyWindow(hwnd) };
        }
        STATE.with(|s| s.borrow_mut().take());
        let _ = unsafe { DeleteObject(self.font.into()) };
    }
}
