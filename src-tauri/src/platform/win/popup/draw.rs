//! Purpose: Layout and software drawing of the tray menu: Raycast's metrics (33 dip rows, 14 dip text, 15 dip icons,
//! full-width separators) painted with GDI into a 32-bit surface that becomes a per-pixel-alpha window, so the corners
//! are anti-aliased and there is no system frame at all.
//! Contents: Palette — light or dark colours; Layout — row positions and window size; Surface — the bitmap; paint —
//! draw everything; hit / next_enabled — pointer and keyboard navigation helpers.

use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;

use super::icons::mask;
use crate::platform::win::shape::coverage;
use crate::platform::MenuEntry;

const ROW: f32 = 33.0; // dip
const SEPARATOR: f32 = 5.0;
const PAD_Y: f32 = 4.0;
const PAD_X: f32 = 14.0;
const ICON: f32 = 15.0;
const GAP: f32 = 7.0;
const MIN_WIDTH: f32 = 200.0;
const RADIUS: f32 = 8.0;
const HOVER_INSET: f32 = 4.0;
const HOVER_RADIUS: f32 = 4.0;
pub const FONT: f32 = 14.0;

pub fn px(dip: f32, scale: f32) -> i32 {
    (dip * scale).round() as i32
}

#[derive(Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    fn colorref(self) -> COLORREF {
        COLORREF(self.0 as u32 | (self.1 as u32) << 8 | (self.2 as u32) << 16)
    }
}

pub struct Palette {
    bg: Rgb,
    hover: Rgb,
    text: Rgb,
    muted: Rgb,
    line: Rgb,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Palette {
                bg: Rgb(44, 44, 44),
                hover: Rgb(61, 61, 61),
                text: Rgb(255, 255, 255),
                muted: Rgb(154, 154, 154),
                line: Rgb(60, 60, 60),
            }
        } else {
            Palette {
                bg: Rgb(249, 249, 249),
                hover: Rgb(234, 234, 234),
                text: Rgb(26, 26, 26),
                muted: Rgb(112, 112, 112),
                line: Rgb(227, 227, 227),
            }
        }
    }
}

pub struct Layout {
    pub scale: f32,
    pub width: i32,
    pub height: i32,
    tops: Vec<i32>,
    heights: Vec<i32>,
}

fn text_width(hdc: HDC, text: &str) -> i32 {
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE::default();
    let _ = unsafe { GetTextExtentPoint32W(hdc, &wide, &mut size) };
    size.cx
}

fn text_start(entry: &MenuEntry, scale: f32) -> i32 {
    let icon = if matches!(entry, MenuEntry::Item { icon: Some(_), .. }) { ICON + GAP } else { 0.0 };
    px(PAD_X + icon, scale)
}

/// `hdc` must have the menu font selected so the text can be measured.
pub fn layout(hdc: HDC, entries: &[MenuEntry], scale: f32) -> Layout {
    let (mut tops, mut heights, mut y, mut widest) = (Vec::new(), Vec::new(), px(PAD_Y, scale), px(MIN_WIDTH, scale));
    for entry in entries {
        let h = px(if matches!(entry, MenuEntry::Separator) { SEPARATOR } else { ROW }, scale);
        if let MenuEntry::Item { label, .. } = entry {
            widest = widest.max(text_start(entry, scale) + text_width(hdc, label) + px(PAD_X, scale));
        }
        tops.push(y);
        heights.push(h);
        y += h;
    }
    Layout { scale, width: widest, height: y + px(PAD_Y, scale), tops, heights }
}

/// Index of the enabled item under client-space `y`.
pub fn hit(layout: &Layout, entries: &[MenuEntry], y: i32) -> Option<usize> {
    let i = layout.tops.iter().zip(&layout.heights).position(|(&top, &h)| y >= top && y < top + h)?;
    matches!(entries[i], MenuEntry::Item { enabled: true, .. }).then_some(i)
}

/// The next enabled item from `from` in direction `step` (+1 or -1), wrapping around.
pub fn next_enabled(entries: &[MenuEntry], from: Option<usize>, step: isize) -> Option<usize> {
    let n = entries.len() as isize;
    let start = from.map_or(if step > 0 { -1 } else { n }, |i| i as isize);
    (1..=n).map(|k| (start + step * k).rem_euclid(n) as usize).find(|&i| matches!(entries[i], MenuEntry::Item { enabled: true, .. }))
}

/// A 32-bit top-down bitmap in a memory DC; its alpha channel becomes the window's per-pixel alpha.
pub struct Surface {
    pub dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u32,
    pub w: i32,
    pub h: i32,
}

impl Surface {
    pub fn new(w: i32, h: i32) -> Option<Self> {
        let header = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        unsafe {
            let dc = CreateCompatibleDC(None);
            let mut bits = std::ptr::null_mut();
            let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
            let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
                let _ = DeleteDC(dc);
                return None;
            };
            SelectObject(dc, bitmap.into());
            Some(Surface { dc, bitmap, bits: bits as *mut u32, w, h })
        }
    }

    fn pixels(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.bitmap.into());
        }
    }
}

fn fill(hdc: HDC, rc: &RECT, color: Rgb) {
    unsafe {
        let brush = CreateSolidBrush(color.colorref());
        FillRect(hdc, rc, brush);
        let _ = DeleteObject(brush.into());
    }
}

fn paint_hover(hdc: HDC, rc: &RECT, scale: f32, color: Rgb) {
    let (inset, d) = (px(HOVER_INSET, scale), px(HOVER_RADIUS, scale) * 2);
    unsafe {
        let brush = CreateSolidBrush(color.colorref());
        let region = CreateRoundRectRgn(rc.left + inset, rc.top + 1, rc.right - inset, rc.bottom - 1, d, d);
        let _ = FillRgn(hdc, region, brush);
        let _ = DeleteObject(region.into());
        let _ = DeleteObject(brush.into());
    }
}

/// Tint the alpha mask with `ink` and alpha-blend it onto `hdc`, centred vertically on `mid_y`.
fn paint_icon(hdc: HDC, name: &str, scale: f32, ink: Rgb, x: i32, mid_y: i32) {
    let Some((alpha, size)) = mask(name, ICON * scale) else { return };
    let s = size as i32;
    let header = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: s,
        biHeight: -s,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    unsafe {
        let mut bits = std::ptr::null_mut();
        let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
        let Ok(bitmap) = CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else { return };
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, alpha.len());
        for (p, &a) in pixels.iter_mut().zip(alpha) {
            let (a, r, g, b) = (a as u32, ink.0 as u32, ink.1 as u32, ink.2 as u32); // premultiplied, as AlphaBlend expects
            *p = a << 24 | (r * a / 255) << 16 | (g * a / 255) << 8 | (b * a / 255);
        }
        let mem = CreateCompatibleDC(Some(hdc));
        let old = SelectObject(mem, bitmap.into());
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        let _ = AlphaBlend(hdc, x, mid_y - s / 2, s, s, mem, 0, 0, s, s, blend);
        SelectObject(mem, old);
        let _ = DeleteDC(mem);
        let _ = DeleteObject(bitmap.into());
    }
}

fn paint_item(s: &Surface, layout: &Layout, pal: &Palette, entry: &MenuEntry, (top, height): (i32, i32), hovered: bool) {
    let rc = RECT { left: 0, top, right: s.w, bottom: top + height };
    let MenuEntry::Item { label, icon, enabled, .. } = entry else {
        return fill(s.dc, &RECT { top: top + height / 2, bottom: top + height / 2 + 1, ..rc }, pal.line);
    };
    if hovered {
        paint_hover(s.dc, &rc, layout.scale, pal.hover);
    }
    let ink = if *enabled { pal.text } else { pal.muted };
    if let Some(name) = icon {
        paint_icon(s.dc, name, layout.scale, ink, px(PAD_X, layout.scale), top + height / 2);
    }
    let mut text_rc = RECT { left: text_start(entry, layout.scale), right: s.w - px(PAD_X, layout.scale), ..rc };
    let mut wide: Vec<u16> = label.encode_utf16().collect();
    unsafe {
        SetBkMode(s.dc, TRANSPARENT);
        SetTextColor(s.dc, ink.colorref());
        DrawTextW(s.dc, &mut wide, &mut text_rc, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX);
    }
}

/// GDI writes colour but not alpha. The card is opaque, so: inside the rounded shape alpha is 255, outside it is 0, and
/// along the curve and the 1 px border the pixel is premultiplied by its coverage. Only corners and edges need maths.
fn shape_alpha(s: &mut Surface, scale: f32, pal: &Palette) {
    let (w, h, r) = (s.w, s.h, RADIUS * scale);
    let corner = r.ceil() as i32 + 1;
    let line = (pal.line.0 as f32, pal.line.1 as f32, pal.line.2 as f32);
    for y in 0..h {
        for x in 0..w {
            let (edge, in_corner) =
                (x == 0 || y == 0 || x == w - 1 || y == h - 1, (x < corner || x >= w - corner) && (y < corner || y >= h - corner));
            let p = &mut s.pixels()[(y * w + x) as usize];
            if !edge && !in_corner {
                *p |= 0xFF00_0000;
                continue;
            }
            let (fx, fy, hw, hh) = (x as f32 + 0.5, y as f32 + 0.5, w as f32 / 2.0, h as f32 / 2.0);
            let outer = coverage(fx, fy, hw, hh, hw, hh, r);
            let inner = coverage(fx, fy, hw, hh, hw - 1.0, hh - 1.0, (r - 1.0).max(0.0));
            let ring = (outer - inner).max(0.0);
            let mix = |c: u32, l: f32| (c as f32 * inner + l * ring).round() as u32;
            *p = ((outer * 255.0).round() as u32) << 24
                | mix((*p >> 16) & 0xFF, line.0) << 16
                | mix((*p >> 8) & 0xFF, line.1) << 8
                | mix(*p & 0xFF, line.2);
        }
    }
}

/// Paint the whole menu. The menu font must already be selected into the surface DC.
pub fn paint(s: &mut Surface, entries: &[MenuEntry], layout: &Layout, pal: &Palette, hover: Option<usize>) {
    fill(s.dc, &RECT { left: 0, top: 0, right: s.w, bottom: s.h }, pal.bg);
    for (i, entry) in entries.iter().enumerate() {
        paint_item(s, layout, pal, entry, (layout.tops[i], layout.heights[i]), hover == Some(i));
    }
    shape_alpha(s, layout.scale, pal);
}
