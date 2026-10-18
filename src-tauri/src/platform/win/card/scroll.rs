//! Purpose: What the card needs to scroll smoothly: the text is drawn ONCE onto a bitmap as tall as all of it, and a wheel turn only copies a window of that bitmap onto the card (a few rows of memory copying, no text drawing), fading it into the card colour at an edge with more text beyond it.
//! Contents: View — built when the card opens (draws the title, then either the whole text or the tall bitmap); `scroll_by` moves the window; `show` copies it, fades the edges and draws the scroll bar; `fix` — one pixel made valid for a layered window.

use windows::Win32::Foundation::POINT;

use super::gdi::{Dib, Fonts};
use super::layout::Laid;
use crate::platform::win::palette::Palette;

/// Dip scrolled by one wheel notch: two lines of text, fine enough to read as it goes.
const NOTCH: f32 = 40.0;

/// Height of the fade band in dip.
const FADE: f32 = 28.0;

/// `base` toward `text` by `t`, channel by channel.
fn mix(base: u32, text: u32, t: f32) -> u32 {
    let channel = |shift: u32| {
        let (b, c) = ((base >> shift) & 255, (text >> shift) & 255);
        (b as f32 + (c as f32 - b as f32) * t).round() as u32 & 255
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

/// `text` (a pixel of the tall bitmap) as the card's pixel `card` (which has its alpha already): a layered window wants colour multiplied by alpha, so the rounded corners at the bottom edge stay transparent.
fn fix(card: u32, text: u32) -> u32 {
    let alpha = card >> 24;
    match alpha {
        255 => 0xff00_0000 | text & 0x00ff_ffff,
        0 => card,
        _ => {
            let scaled = |shift: u32| ((text >> shift) & 255) * alpha / 255;
            alpha << 24 | scaled(16) << 16 | scaled(8) << 8 | scaled(0)
        }
    }
}

pub(super) struct View {
    fonts: Fonts,
    laid: Laid,
    palette: Palette,
    margin: i32,
    /// All of the text on a bitmap `laid.width` by `content - body_top`, made once; None when the text fits on the card or the bitmap could not be made.
    surface: Option<Dib>,
    offset: i32,
    /// DPI scale.
    scale: f32,
    /// Where the bitmap goes on the screen.
    pub at: POINT,
}

impl View {
    /// Draws the title and the text onto `dib` (the card painted without text) and puts back its `alpha`. `look` is the margin around the card in the bitmap and the DPI scale.
    pub fn new(fonts: Fonts, laid: Laid, palette: Palette, alpha: Vec<u8>, dib: &Dib, at: POINT, look: (i32, f32)) -> View {
        let (margin, scale) = look;
        // SAFETY: GDI drawing on bitmaps made on this thread.
        let surface = unsafe {
            fonts.header(dib, &laid, margin, &palette);
            let tall = (laid.content > laid.height).then(|| Self::render(&fonts, &laid, &palette)).flatten();
            if tall.is_none() {
                fonts.body(dib.dc, &laid, &palette, (margin, margin));
            }
            tall
        };
        dib.restore_alpha(&alpha);
        let view = View { fonts, laid, palette, margin, surface, offset: 0, scale, at };
        view.show(dib);
        view
    }

    unsafe fn render(fonts: &Fonts, laid: &Laid, palette: &Palette) -> Option<Dib> {
        let surface = Dib::new(laid.width, laid.content - laid.body_top)?;
        fonts.fill(surface.dc, (0, 0, surface.w, surface.h), palette.card);
        fonts.body(surface.dc, laid, palette, (0, -laid.body_top));
        Some(surface)
    }

    /// Copies the visible window of the tall bitmap onto the card, below the title. Rows within the fade band of an edge that has more text beyond it are blended into the card colour (text meeting an edge dissolves instead of being cut off).
    fn show(&self, dib: &Dib) {
        let Some(surface) = &self.surface else { return };
        let (rows, band) = (self.laid.height - self.laid.body_top, (FADE * self.scale).round() as i32);
        let (fade_top, fade_bottom) = (self.offset > 0, self.offset < self.laid.content - self.laid.height);
        let (r, g, b) = self.palette.card.bytes();
        let card = (r as u32) << 16 | (g as u32) << 8 | b as u32;
        let (width, to_x, top) = (self.laid.width as usize, self.margin as usize, (self.margin + self.laid.body_top) as usize);
        surface.with_pixels(|src| {
            dib.with_pixels(|dst| {
                for row in 0..rows {
                    let edge = [fade_top.then_some(row), fade_bottom.then_some(rows - 1 - row)].into_iter().flatten().min().unwrap_or(band);
                    let t = (edge as f32 / band as f32).min(1.0);
                    let from = (self.offset + row) as usize * width;
                    let to = (top + row as usize) * dib.w as usize + to_x;
                    for (d, s) in dst[to..to + width].iter_mut().zip(&src[from..from + width]) {
                        *d = fix(*d, if t < 1.0 { mix(card, *s, t) } else { *s });
                    }
                }
            })
        });
        // SAFETY: GDI drawing on the card's bitmap, after the copy so the bar is not dimmed by the fade.
        unsafe { self.fonts.thumb(dib, &self.laid, self.margin, &self.palette, self.offset) };
    }

    /// Scrolls by a wheel `delta` (120 a notch, positive is up) and copies the new window; false when it did not move.
    pub fn scroll_by(&mut self, dib: &Dib, delta: i32) -> bool {
        // Proportional to the wheel's own figure (120 a notch, less for a touchpad or a high-resolution wheel) and rounded, so small turns still move.
        let moved = (delta as f32 * NOTCH * self.scale / 120.0).round() as i32;
        let to = (self.offset - moved).clamp(0, (self.laid.content - self.laid.height).max(0));
        if self.surface.is_none() || to == self.offset {
            return false;
        }
        self.offset = to;
        self.show(dib);
        true
    }
}
