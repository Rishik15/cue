//! Purpose: The overlay's message pill ("Download a model to start dictating"): same look and theme as the recording pill, sized to its text.
//! Contents: font — the message font; width — pill width for a text; draw — centred text on a painted pill.

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;

use super::palette::Rgb;

const FONT: f32 = 13.0; // dip
pub const PAD_X: f32 = 20.0;

pub unsafe fn font(scale: f32) -> HFONT {
    let height = -((FONT * scale).round() as i32);
    CreateFontW(
        height,
        0,
        0,
        0,
        500,
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

/// Pill width in pixels: the text plus padding, never narrower than the recording pill.
pub unsafe fn width(text: &str, font: HFONT, scale: f32, at_least: i32) -> i32 {
    let dc = CreateCompatibleDC(None);
    let old = SelectObject(dc, font.into());
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE::default();
    let _ = GetTextExtentPoint32W(dc, &wide, &mut size);
    SelectObject(dc, old);
    let _ = DeleteDC(dc);
    at_least.max(size.cx + (2.0 * PAD_X * scale).round() as i32)
}

/// Centre `text` in `area` of the pill already painted into `dc`.
pub unsafe fn draw(dc: HDC, text: &str, font: HFONT, area: &mut RECT, ink: Rgb) {
    let old = SelectObject(dc, font.into());
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, COLORREF(ink.colorref()));
    DrawTextW(dc, &mut wide, area, DT_SINGLELINE | DT_VCENTER | DT_CENTER | DT_END_ELLIPSIS | DT_NOPREFIX);
    SelectObject(dc, old);
}
