//! Purpose: What the card needs to repaint itself when it scrolls: the fonts, the laid-out text and the card's pixels as they were before any text was drawn.
//! Contents: View — built when the card opens; `scroll_by` moves the text and repaints it; `paint` draws the text over the saved card and fades it out into the card colour at an edge with more text beyond it.

use windows::Win32::Foundation::POINT;

use super::gdi::{Dib, Fonts};
use super::layout::Laid;
use crate::platform::win::palette::Palette;

/// Dip scrolled by one wheel notch: two lines of text, fine enough to read as it goes.
const NOTCH: f32 = 40.0;

/// Height of the fade band in dip.
const FADE: f32 = 28.0;

/// `base` toward `text` by `t`, channel by channel (alpha is put back afterwards).
fn mix(base: u32, text: u32, t: f32) -> u32 {
    let channel = |shift: u32| {
        let (b, c) = ((base >> shift) & 255, (text >> shift) & 255);
        (b as f32 + (c as f32 - b as f32) * t).round() as u32 & 255
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

pub(super) struct View {
    fonts: Fonts,
    laid: Laid,
    palette: Palette,
    margin: i32,
    /// The card's alpha channel, put back after GDI text zeroes it.
    alpha: Vec<u8>,
    /// The card without text, kept only when the text is taller than the card.
    base: Vec<u32>,
    offset: i32,
    /// Pixels per wheel notch.
    scale: f32,
    /// Where the bitmap goes on the screen.
    pub at: POINT,
}

impl View {
    /// `look` is the margin around the card in the bitmap and the DPI scale.
    pub fn new(fonts: Fonts, laid: Laid, palette: Palette, alpha: Vec<u8>, dib: &Dib, at: POINT, look: (i32, f32)) -> View {
        let mut base = Vec::new();
        if laid.content > laid.height {
            dib.with_pixels(|pixels| base = pixels.to_vec());
        }
        View { fonts, laid, palette, margin: look.0, alpha, base, offset: 0, scale: look.1, at }
    }

    /// Text over the card at the current offset.
    pub unsafe fn paint(&self, dib: &Dib) {
        if !self.base.is_empty() {
            dib.with_pixels(|pixels| pixels.copy_from_slice(&self.base));
        }
        self.fonts.draw(dib, &self.laid, self.margin, &self.palette, self.offset);
        if !self.base.is_empty() {
            self.fade(dib);
            // after the fade, so the bar is not dimmed by it
            self.fonts.thumb(dib, &self.laid, self.margin, &self.palette, self.offset);
        }
        dib.restore_alpha(&self.alpha);
    }

    /// Text meeting an edge (the bottom of the title, the bottom of the card) dissolves into the card over a band, instead of being cut off: top rows blend from the plain card (edge) to the text (inside), bottom rows the other way.
    fn fade(&self, dib: &Dib) {
        let band = (FADE * self.scale).round() as i32;
        let (width, top, bottom) = (dib.w as usize, self.margin + self.laid.body_top, self.margin + self.laid.height);
        let rows_top = if self.offset > 0 { 0..band } else { 0..0 };
        let rows_bottom = if self.offset < self.laid.content - self.laid.height { 0..band } else { 0..0 };
        dib.with_pixels(|pixels| {
            let mut blend = |row: i32, t: f32| {
                let at = row as usize * width;
                for x in self.margin as usize..(self.margin + self.laid.width) as usize {
                    pixels[at + x] = mix(self.base[at + x], pixels[at + x], t);
                }
            };
            rows_top.for_each(|i| blend(top + i, i as f32 / band as f32));
            rows_bottom.for_each(|i| blend(bottom - 1 - i, i as f32 / band as f32));
        });
    }

    /// Scrolls by a wheel `delta` (120 a notch, positive is up) and repaints; false when it did not move.
    pub unsafe fn scroll_by(&mut self, dib: &Dib, delta: i32) -> bool {
        // Proportional to the wheel's own figure (120 a notch, less for a touchpad or a high-resolution wheel) and rounded, so small turns still move.
        let moved = (delta as f32 * NOTCH * self.scale / 120.0).round() as i32;
        let to = (self.offset - moved).clamp(0, (self.laid.content - self.laid.height).max(0));
        if to == self.offset {
            return false;
        }
        self.offset = to;
        self.paint(dib);
        true
    }
}
