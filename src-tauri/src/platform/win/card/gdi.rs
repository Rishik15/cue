//! Purpose: The card's GDI pieces: one font per text style (measuring and drawing) and the 32-bit bitmap the layered window shows.
//! Contents: Fonts — create, measure and draw text in the styles `layout` names; Dib — the bitmap, its pixels and the alpha repair after GDI text.

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;

use super::layout::{Laid, BODY, EXAMPLE, LABEL, NUMBER, TITLE, WORD};
use crate::platform::win::palette::Palette;

/// The text styles as GDI fonts.
pub(super) struct Fonts {
    handles: [HFONT; 6],
    dc: HDC,
}

impl Fonts {
    pub unsafe fn new(scale: f32) -> Fonts {
        let make = |dip: f32, weight: i32, italic: bool, face: windows::core::PCWSTR| {
            CreateFontW(
                -((dip * scale).round() as i32),
                0,
                0,
                0,
                weight,
                italic as u32,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                face,
            )
        };
        let ui = w!("Segoe UI Variable Text");
        let handles = [
            make(13.5, 400, false, ui),            // BODY
            make(13.0, 400, true, ui),             // EXAMPLE
            make(12.5, 400, true, ui),             // LABEL
            make(21.0, 700, false, w!("Georgia")), // WORD
            make(11.5, 600, false, ui),            // NUMBER
            make(13.0, 400, false, ui),            // TITLE
        ];
        Fonts { handles, dc: CreateCompatibleDC(None) }
    }

    pub unsafe fn width(&self, text: &str, style: u8) -> i32 {
        let old = SelectObject(self.dc, self.handles[style as usize].into());
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(self.dc, &wide, &mut size);
        SelectObject(self.dc, old);
        size.cx
    }

    /// Text and the hairline after the title, over the painted card.
    pub unsafe fn draw(&self, dib: &Dib, laid: &Laid, margin: i32, palette: &Palette) {
        let dc = dib.dc;
        SetBkMode(dc, TRANSPARENT);
        let ink = |style: u8| match style {
            BODY | WORD => palette.ink,
            EXAMPLE => palette.ink.mix(palette.muted, 0.45),
            LABEL | NUMBER | TITLE => palette.muted,
            _ => palette.ink,
        };
        for d in &laid.draws {
            let old = SelectObject(dc, self.handles[d.style as usize].into());
            SetTextColor(dc, COLORREF(ink(d.style).colorref()));
            let wide: Vec<u16> = d.text.encode_utf16().collect();
            let _ = TextOutW(dc, margin + d.x, margin + d.y, &wide);
            SelectObject(dc, old);
        }
        let (from, to, y) = laid.rule;
        let brush = CreateSolidBrush(COLORREF(palette.line().colorref()));
        FillRect(dc, &RECT { left: margin + from, top: margin + y, right: margin + to, bottom: margin + y + 1 }, brush);
        let _ = DeleteObject(brush.into());
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
