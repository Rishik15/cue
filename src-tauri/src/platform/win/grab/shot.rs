//! File: shot.rs
//! Purpose: A copy of the whole desktop (every monitor) held as a GDI bitmap, with the pieces the selector and the cropper need.
//! Contents: Shot — the copy (freed on drop); take — copy the screen (refused when the copy plus the selector's paint canvas pass a byte budget); paint — draw part of it, dimmed, with the chosen rectangle lit and framed in the accent colour (composed off screen to avoid flicker); copy_scaled — a rectangle as BGRA bytes, enlarged by a whole factor; surface / bitmap_info — a memory DC with a 32-bit bitmap; intersect — two rectangles overlapped.
//! shortcut: the memory-DC-plus-DIB-section allocation and cleanup here (`surface`) is repeated in `card/gdi.rs`, `overlay/canvas.rs` and `popup/draw.rs`; one shared owned-bitmap type would keep it in one place, upgrade when a fifth copy is needed or one of the four changes.

use crate::platform::win::palette::Rgb;
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

/// The copy of the desktop: a memory DC holding a bitmap, freed on drop.
pub struct Shot {
    /// The memory DC.
    dc: HDC,
    /// The bitmap selected into the DC.
    bitmap: HBITMAP,
    /// What the DC held before the bitmap.
    old: HGDIOBJ,
    /// Where the desktop's top-left corner is on the virtual screen, and its size.
    pub x: i32,
    /// Top edge on the virtual screen.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
    /// Colour of the frame around the lit rectangle (the theme accent).
    frame: COLORREF,
}

/// The header of a 32-bit top-down bitmap `w` by `h`.
fn bitmap_info(w: i32, h: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A memory DC with a new 32-bit top-down bitmap selected into it, and a pointer to its pixels. Safety: the caller owns the returned handles and frees them; on failure nothing is left allocated.
unsafe fn surface(w: i32, h: i32) -> Option<(HDC, HBITMAP, HGDIOBJ, *const u8)> {
    let dc = CreateCompatibleDC(None);
    let mut bits = std::ptr::null_mut();
    let Ok(bitmap) = CreateDIBSection(Some(dc), &bitmap_info(w, h), DIB_RGB_COLORS, &mut bits, None, 0) else {
        let _ = DeleteDC(dc);
        return None;
    };
    Some((dc, bitmap, SelectObject(dc, bitmap.into()), bits as *const u8))
}

impl Shot {
    /// Copies the whole desktop; `frame` is the colour of the frame around the lit rectangle, `budget` the most bytes the copy may take (checked before anything is allocated). Err with a line for the user when the desktop is larger than that or could not be copied. Safety: GDI calls on handles created and freed here; a failure frees what was made.
    pub fn take(frame: Rgb, budget: usize) -> Result<Shot, String> {
        unsafe {
            let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
            let bytes = held_while_painting(w, h);
            if bytes.is_none_or(|b| b > budget) {
                return Err("Your screen layout is too large for Grab Text to copy.".into());
            }
            let failed = || "Cue could not copy the screen to read it.".to_string();
            let (dc, bitmap, old, _) = surface(w, h).ok_or_else(failed)?;
            let screen = GetDC(None);
            let copied = BitBlt(dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY | CAPTUREBLT).is_ok();
            ReleaseDC(None, screen);
            let shot = Shot { dc, bitmap, old, x, y, w, h, frame: COLORREF(frame.colorref()) };
            if copied {
                Ok(shot)
            } else {
                Err(failed())
            }
        }
    }

    /// Draws `area` (window coordinates, which are desktop-copy coordinates) onto `target`: the picture, dimmed unless it lies inside `lit`, which is undimmed and framed. It is composed off screen and copied once: drawing the layers straight onto the window shows each of them for a moment, which flickers.
    /// Safety: `target` is the window's paint DC; every object made here is freed before returning.
    pub fn paint(&self, target: HDC, area: RECT, lit: Option<RECT>) {
        unsafe {
            let (aw, ah) = (area.right - area.left, area.bottom - area.top);
            let (buffer, canvas) = (CreateCompatibleDC(Some(target)), CreateCompatibleBitmap(target, aw, ah));
            let old_canvas = SelectObject(buffer, canvas.into());
            let _ = SetWindowOrgEx(buffer, area.left, area.top, None);
            let _ = BitBlt(buffer, area.left, area.top, aw, ah, Some(self.dc), area.left, area.top, SRCCOPY);
            let black = CreateCompatibleDC(None);
            let pixel = CreateCompatibleBitmap(target, 1, 1);
            let old = SelectObject(black, pixel.into());
            let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 140, AlphaFormat: 0 };
            let _ = PatBlt(black, 0, 0, 1, 1, BLACKNESS);
            let _ = AlphaBlend(buffer, area.left, area.top, aw, ah, black, 0, 0, 1, 1, blend);
            SelectObject(black, old);
            let _ = DeleteObject(pixel.into());
            let _ = DeleteDC(black);
            if let Some(l) = lit.and_then(|l| intersect(l, area)) {
                let _ = BitBlt(buffer, l.left, l.top, l.right - l.left, l.bottom - l.top, Some(self.dc), l.left, l.top, SRCCOPY);
                let brush = CreateSolidBrush(self.frame);
                FrameRect(buffer, &l, brush);
                let _ = DeleteObject(brush.into());
            }
            let _ = BitBlt(target, area.left, area.top, aw, ah, Some(buffer), area.left, area.top, SRCCOPY);
            SelectObject(buffer, old_canvas);
            let _ = DeleteObject(canvas.into());
            let _ = DeleteDC(buffer);
        }
    }

    /// The pixels of `r` (desktop-copy coordinates) enlarged `k` times, as BGRA bytes with the alpha set opaque. Safety: a scratch bitmap of the output size (None when its byte size overflows), read through its own pixel pointer after `GdiFlush` has finished the batched blit, and freed before returning.
    pub fn copy_scaled(&self, r: RECT, k: i32) -> Option<Vec<u8>> {
        let (w, h) = ((r.right - r.left).checked_mul(k)?, (r.bottom - r.top).checked_mul(k)?);
        let len = usize::try_from(w).ok()?.checked_mul(usize::try_from(h).ok()?)?.checked_mul(4)?;
        unsafe {
            let (dc, bitmap, old, bits) = surface(w, h)?;
            let _ = SetStretchBltMode(dc, HALFTONE);
            let _ = SetBrushOrgEx(dc, 0, 0, None);
            let ok = StretchBlt(dc, 0, 0, w, h, Some(self.dc), r.left, r.top, r.right - r.left, r.bottom - r.top, SRCCOPY).as_bool();
            let _ = GdiFlush();
            let mut out = std::slice::from_raw_parts(bits, len).to_vec();
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
            out.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
            ok.then_some(out)
        }
    }
}

/// The overlap of two rectangles; None when they do not overlap.
fn intersect(a: RECT, b: RECT) -> Option<RECT> {
    let r = RECT { left: a.left.max(b.left), top: a.top.max(b.top), right: a.right.min(b.right), bottom: a.bottom.min(b.bottom) };
    (r.right > r.left && r.bottom > r.top).then_some(r)
}

impl Drop for Shot {
    /// Frees the bitmap and the DC. Safety: the handles were made by `take` and are freed once.
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

/// The bytes held while the selector paints: the desktop copy and, for a repaint of the whole window, a canvas as large (`paint` composes off screen). None for an empty or overflowing layout, which is refused.
fn held_while_painting(w: i32, h: i32) -> Option<usize> {
    let one = usize::try_from(w).ok()?.checked_mul(usize::try_from(h).ok()?)?.checked_mul(4)?;
    Some(one).filter(|b| *b > 0)?.checked_mul(2)
}

#[cfg(test)]
mod tests {
    use super::held_while_painting;

    /// The desktop copy counts twice (copy and full-window paint canvas); an empty or absurd layout has no size.
    #[test]
    fn the_paint_canvas_counts_against_the_budget() {
        assert_eq!(held_while_painting(1920, 1080), Some(1920 * 1080 * 4 * 2));
        assert_eq!(held_while_painting(0, 1080), None);
        assert_eq!(held_while_painting(-5, 10), None);
        assert_eq!(held_while_painting(i32::MAX, i32::MAX), None);
    }
}
