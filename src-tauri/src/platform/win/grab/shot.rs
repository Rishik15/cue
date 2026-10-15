//! Purpose: A copy of the whole desktop (every monitor) held as a GDI bitmap, with the pieces the selector and the cropper need.
//! Contents: Shot — the copy (freed on drop); take — copy the screen; paint — draw part of it, dimmed, with the chosen rectangle lit and framed (composed off screen to avoid flicker); copy_scaled — a rectangle as BGRA bytes, enlarged by a whole factor.

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

pub struct Shot {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    /// Where the desktop's top-left corner is on the virtual screen, and its size.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

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

/// A memory DC with a new 32-bit top-down bitmap selected into it, and a pointer to its pixels.
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
    pub fn take() -> Option<Shot> {
        // SAFETY: GDI calls on handles created and freed here; a failure frees what was made and returns None.
        unsafe {
            let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
            if w <= 0 || h <= 0 {
                return None;
            }
            let (dc, bitmap, old, _) = surface(w, h)?;
            let screen = GetDC(None);
            let copied = BitBlt(dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY | CAPTUREBLT).is_ok();
            ReleaseDC(None, screen);
            let shot = Shot { dc, bitmap, old, x, y, w, h };
            copied.then_some(shot)
        }
    }

    /// Draws `area` (window coordinates, which are desktop-copy coordinates) onto `target`: the picture, dimmed unless it lies inside `lit`.
    /// Draws `area` of the dimmed screen with `lit` undimmed and framed. It is composed off screen and copied once: drawing the layers straight onto the window shows each of them for a moment, which flickers.
    pub fn paint(&self, target: HDC, area: RECT, lit: Option<RECT>) {
        // SAFETY: `target` is the window's paint DC; every object made here is freed before returning.
        unsafe {
            let (aw, ah) = (area.right - area.left, area.bottom - area.top);
            let (buffer, canvas) = (CreateCompatibleDC(Some(target)), CreateCompatibleBitmap(target, aw, ah));
            let old_canvas = SelectObject(buffer, canvas.into());
            let _ = SetWindowOrgEx(buffer, area.left, area.top, None); // the same coordinates as the window
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
                FrameRect(buffer, &l, HBRUSH(GetStockObject(WHITE_BRUSH).0));
            }
            let _ = BitBlt(target, area.left, area.top, aw, ah, Some(buffer), area.left, area.top, SRCCOPY);
            SelectObject(buffer, old_canvas);
            let _ = DeleteObject(canvas.into());
            let _ = DeleteDC(buffer);
        }
    }

    /// The pixels of `r` (desktop-copy coordinates) enlarged `k` times, as BGRA bytes with the alpha set opaque.
    pub fn copy_scaled(&self, r: RECT, k: i32) -> Option<Vec<u8>> {
        let (w, h) = ((r.right - r.left) * k, (r.bottom - r.top) * k);
        // SAFETY: a scratch bitmap of the output size, read through its own pixel pointer and freed before returning.
        unsafe {
            let (dc, bitmap, old, bits) = surface(w, h)?;
            let _ = SetStretchBltMode(dc, HALFTONE);
            let _ = SetBrushOrgEx(dc, 0, 0, None);
            let ok = StretchBlt(dc, 0, 0, w, h, Some(self.dc), r.left, r.top, r.right - r.left, r.bottom - r.top, SRCCOPY).as_bool();
            let mut out = std::slice::from_raw_parts(bits, (w * h * 4) as usize).to_vec();
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
            out.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
            ok.then_some(out)
        }
    }
}

fn intersect(a: RECT, b: RECT) -> Option<RECT> {
    let r = RECT { left: a.left.max(b.left), top: a.top.max(b.top), right: a.right.min(b.right), bottom: a.bottom.min(b.bottom) };
    (r.right > r.left && r.bottom > r.top).then_some(r)
}

impl Drop for Shot {
    fn drop(&mut self) {
        // SAFETY: the handles were made by `take` and are freed once.
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}
