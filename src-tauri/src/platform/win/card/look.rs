//! Purpose: The card's look: its measurements in dip and the painting of its surface (rounded body, arrow, light rim and soft edge) into the bitmap.
//! Contents: the size constants; paint — fill the bitmap and return the alpha channel to restore after the text is drawn over it.

use super::gdi::Dib;
use super::layout::Laid;
use super::shape::{render, Arrow, Spec};
use crate::platform::place::Side;
use crate::platform::win::palette::{Palette, Rgb};

/// Sizes in dip: the space around the card for its shadow, the card's corner, the arrow, and how near a corner the arrow may sit.
pub const MARGIN: f32 = 14.0;
const RADIUS: f32 = 16.0;
const ARROW_HALF: f32 = 10.0;
pub const ARROW_REACH: f32 = 8.0;
pub const ARROW_GAP: f32 = 3.0;
pub const ARROW_INSET: f32 = 26.0;

/// The card, its arrow, border and shadow into the bitmap; returns the alpha channel to restore after the text is drawn over it.
pub(super) unsafe fn paint(dib: &Dib, laid: &Laid, side: Side, offset: i32, palette: &Palette, margin: i32, scale: f32) -> Vec<u8> {
    let bytes = |c: Rgb| {
        let (r, g, b) = c.bytes();
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
    };
    let spec = Spec {
        width: dib.w as usize,
        height: dib.h as usize,
        card: (margin as f32, margin as f32, laid.width as f32, laid.height as f32),
        radius: RADIUS * scale,
        arrow: Some(Arrow { side, offset: offset as f32, half_width: ARROW_HALF * scale, reach: ARROW_REACH * scale }),
        fill: bytes(palette.card),
        border: bytes(palette.border()),
        border_width: 0.0, // a flat card, no outline
        shadow_dy: 2.0 * scale,
        shadow_blur: 9.0 * scale,
        shadow_alpha: 0.16,
    };
    let mut alpha = Vec::new();
    dib.with_pixels(|pixels| alpha = render(&spec, pixels));
    alpha
}
