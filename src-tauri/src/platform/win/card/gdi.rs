//! Purpose: The card's GDI pieces: one font per text style (measuring and drawing) and the 32-bit bitmap the layered window shows.
//! Contents: Fonts — create, measure and draw text in the styles `layout` names (the pinned title, the body onto any bitmap, and a thin scroll thumb); Dib — the bitmap, its pixels and the alpha repair after GDI text.

use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;

use super::layout::{Laid, BODY, WORD};
use crate::platform::win::fonts::Private;
use crate::platform::win::palette::{Palette, Rgb};

/// The text styles as GDI fonts.
pub(super) struct Fonts {
    handles: [HFONT; 6],
    dc: HDC,
    scale: f32,
    /// Keeps Inter registered for as long as the card's fonts exist.
    _inter: Private,
}

impl Fonts {
    pub unsafe fn new(scale: f32) -> Fonts {
        let inter = Private::load();
        // Inter everywhere; the example is told apart by Inter Italic and a muted colour.
        let handles = [
            inter.create(14.0, scale, 400),   // BODY
            inter.create_italic(14.0, scale), // EXAMPLE
            inter.create(12.3, scale, 400),   // LABEL
            inter.create(20.0, scale, 600),   // WORD
            inter.create(12.3, scale, 400),   // NUMBER
            inter.create(13.3, scale, 500),   // TITLE
        ];
        Fonts { handles, dc: CreateCompatibleDC(None), scale, _inter: inter }
    }

    pub unsafe fn width(&self, text: &str, style: u8) -> i32 {
        let old = SelectObject(self.dc, self.handles[style as usize].into());
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(self.dc, &wide, &mut size);
        SelectObject(self.dc, old);
        size.cx
    }

    /// The title and its hairline, which stay where they are however the card scrolls.
    pub unsafe fn header(&self, dib: &Dib, laid: &Laid, margin: i32, palette: &Palette) {
        SetBkMode(dib.dc, TRANSPARENT);
        self.text(dib.dc, palette, &laid.draws[0], (margin, margin));
        let (from, to, y) = laid.rule;
        let thick = self.px(1.5).max(1);
        self.fill(
            dib.dc,
            (margin + from, margin + y - thick / 2, margin + to, margin + y - thick / 2 + thick),
            palette.card.mix(palette.ink, 0.16),
        );
    }

    /// Everything after the title, with `origin` as the card's top-left corner in `dc`.
    pub unsafe fn body(&self, dc: HDC, laid: &Laid, palette: &Palette, origin: (i32, i32)) {
        SetBkMode(dc, TRANSPARENT);
        laid.draws.iter().skip(1).for_each(|d| self.text(dc, palette, d, origin));
    }

    unsafe fn text(&self, dc: HDC, palette: &Palette, d: &super::layout::Draw, origin: (i32, i32)) {
        let ink = match d.style {
            BODY | WORD => palette.ink,
            _ => palette.muted,
        };
        let old = SelectObject(dc, self.handles[d.style as usize].into());
        SetTextColor(dc, COLORREF(ink.colorref()));
        let wide: Vec<u16> = d.text.encode_utf16().collect();
        let _ = TextOutW(dc, origin.0 + d.x, origin.1 + d.y, &wide);
        SelectObject(dc, old);
    }

    fn px(&self, dip: f32) -> i32 {
        (dip * self.scale).round() as i32
    }

    pub unsafe fn fill(&self, dc: HDC, (left, top, right, bottom): (i32, i32, i32, i32), color: Rgb) {
        let brush = CreateSolidBrush(COLORREF(color.colorref()));
        FillRect(dc, &RECT { left, top, right, bottom }, brush);
        let _ = DeleteObject(brush.into());
    }

    /// The thin bar at the right edge showing how far the body has scrolled: left, top, right, bottom in the bitmap. Below the title, short of the rounded corner, a quiet marker and not a control.
    pub fn thumb(&self, laid: &Laid, margin: i32, offset: i32) -> (i32, i32, i32, i32) {
        let (track_top, track) = (laid.body_top + self.px(6.0), laid.height - laid.body_top - self.px(22.0));
        let size = (track * (laid.height - laid.body_top) / (laid.content - laid.body_top)).max(self.px(24.0));
        let y = margin + track_top + (track - size) * offset / (laid.content - laid.height);
        let x = margin + laid.width - self.px(9.0);
        (x, y, x + self.px(2.0), y + size)
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            self.handles.iter().for_each(|f| {
                let _ = DeleteObject((*f).into());
            });
            let _ = DeleteDC(self.dc);
        }
    }
}

/// A 32-bit top-down bitmap in a memory DC: the pixels of the layered window.
pub(super) struct Dib {
    pub dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u32,
    pub w: i32,
    pub h: i32,
}

impl Dib {
    pub unsafe fn new(w: i32, h: i32) -> Option<Dib> {
        let dc = CreateCompatibleDC(None);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            return None;
        };
        SelectObject(dc, bitmap.into());
        Some(Dib { dc, bitmap, bits: bits as *mut u32, w, h })
    }

    pub fn with_pixels(&self, f: impl FnOnce(&mut [u32])) {
        // GDI batches its drawing: whatever it still has pending for the bitmap lands before the pixels are read or changed here.
        let _ = unsafe { GdiFlush() };
        // SAFETY: the section is `w * h` pixels and lives as long as `self`; only the card thread touches it.
        f(unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) });
    }

    /// GDI draws text with a zero alpha byte; the card and its shadow had the right alpha before, so it goes back.
    pub fn restore_alpha(&self, alpha: &[u8]) {
        self.with_pixels(|pixels| pixels.iter_mut().zip(alpha).for_each(|(p, a)| *p = (*p & 0x00ff_ffff) | (*a as u32) << 24));
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.bitmap.into());
        }
    }
}
