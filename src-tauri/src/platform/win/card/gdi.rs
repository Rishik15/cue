//! File: gdi.rs
//! Purpose: The card's GDI pieces: one font per text style (measuring and drawing) and the 32-bit bitmap the layered window shows.
//! Contents:
//! - Fonts — create, measure and draw text in the styles `layout` names (the pinned title, the body onto any bitmap, and a thin scroll thumb)
//! - Dib — the bitmap, its pixels and the alpha repair after GDI text

use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;

use super::layout::{Laid, BODY, TITLE, WORD};
use crate::platform::win::fonts::Private;
use crate::platform::win::palette::{Palette, Rgb};

/// The text styles as GDI fonts.
pub(super) struct Fonts {
    handles: [HFONT; 7],
    dc: HDC,
    scale: f32,
    /// Keeps Inter registered for as long as the card's fonts exist.
    _inter: Private,
}

impl Fonts {
    /// Creates one GDI font per text style at `scale`, with a DC to measure in. Return: the fonts; they are deleted on drop. Must run on the card thread. Order: body, example, label, word, number, title, part of speech. Inter everywhere; the example is told apart by Inter Italic and a muted colour.
    pub unsafe fn new(scale: f32) -> Fonts {
        let inter = Private::load();
        let handles = [
            inter.create(13.0, scale, 400),
            inter.create_italic(12.0, scale),
            inter.create(12.0, scale, 400),
            inter.create(16.5, scale, 600),
            inter.create(12.0, scale, 400),
            inter.create(16.0, scale, 400),
            inter.create(12.0, scale, 400),
        ];
        Fonts { handles, dc: CreateCompatibleDC(None), scale, _inter: inter }
    }

    /// Width in px of `text` in text style `style` (an index into the fonts). Must run on the card thread; `style` must be a valid style.
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

    /// Draws one laid-out piece of text with GDI at `origin` in the tone its style calls for. Input: the target DC, the palette, the piece and its origin. `dc` must be a live DC. Three tones: the headword strongest, the title and the meanings a little softer, the rest (examples, numbers, parts of speech) muted.
    unsafe fn text(&self, dc: HDC, palette: &Palette, d: &super::layout::Draw, origin: (i32, i32)) {
        let ink = match d.style {
            WORD => palette.ink,
            BODY => palette.ink.mix(palette.muted, 0.15),
            TITLE => palette.ink.mix(palette.muted, 0.25),
            _ => palette.muted,
        };
        let old = SelectObject(dc, self.handles[d.style as usize].into());
        SetTextColor(dc, COLORREF(ink.colorref()));
        let wide: Vec<u16> = d.text.encode_utf16().collect();
        let _ = TextOutW(dc, origin.0 + d.x, origin.1 + d.y, &wide);
        SelectObject(dc, old);
    }

    /// Converts dip to px at the card's scale.
    fn px(&self, dip: f32) -> i32 {
        (dip * self.scale).round() as i32
    }

    /// Fills a rectangle (left, top, right, bottom in px) with a solid colour. `dc` must be a live DC.
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
    /// Deletes the fonts and the measuring DC. Safety: the handles were created by this value, are deleted once here, and nothing uses them afterwards.
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
    /// Creates a top-down 32-bit DIB section of `w` by `h` px in a memory DC. Return: None when GDI refuses. The DC and bitmap are released on drop. Safety: GDI calls on handles this value owns; the bits pointer stays valid while the bitmap is alive.
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

    /// Runs `f` on the bitmap's pixels (BGRA, premultiplied) after GDI has flushed its pending drawing. Input: the closure. Only the card thread may call it. Safety: The section is `w * h` pixels and lives as long as `self`; only the card thread touches it. GDI batches its drawing: whatever it still has pending for the bitmap lands before the pixels are read or changed here.
    pub fn with_pixels(&self, f: impl FnOnce(&mut [u32])) {
        let _ = unsafe { GdiFlush() };
        f(unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) });
    }

    /// GDI draws text with a zero alpha byte; the card and its shadow had the right alpha before, so it goes back.
    pub fn restore_alpha(&self, alpha: &[u8]) {
        self.with_pixels(|pixels| pixels.iter_mut().zip(alpha).for_each(|(p, a)| *p = (*p & 0x00ff_ffff) | (*a as u32) << 24));
    }
}

impl Drop for Dib {
    /// Deletes the bitmap and its DC. Safety: the handles were created by this value, are deleted once here, and nothing uses them afterwards.
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.bitmap.into());
        }
    }
}
