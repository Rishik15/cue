//! File: shadow.rs
//! Purpose: The soft shadow round the tray menu, like the one Windows gives its own menus. The menu is a layered window drawn by the app, which gets no shadow from the system, so the window is made a few pixels larger on every side and the shadow is painted into that margin: a dark halo that fades with the distance from the card, shifted a little downwards. It is computed once when the menu opens and only copied on a repaint, so there is no cost while no menu is open.
//! Contents: margin — the width of the shadow margin in px; render — the shadow pixels for a card; compose — the card over its shadow into the window's frame.

use crate::platform::win::shape::distance;

/// Width of the margin the shadow fades out in, in dip.
const MARGIN: f32 = 14.0;
/// How far the shadow is shifted down, in dip.
const OFFSET_Y: f32 = 4.0;
/// Darkest shadow alpha right at the card's edge (0..1).
const STRENGTH: f32 = 0.32;

/// The margin on every side of the card. Input: the display scale. Return: px.
pub fn margin(scale: f32) -> i32 {
    (MARGIN * scale).round() as i32
}

/// The shadow of a card as premultiplied black pixels (BGRA, alpha only), `(w + 2m) * (h + 2m)` of them, row by row. Input: the card size, the margin, the card's corner radius (all px) and the display scale. Return: the pixels; the part under the card is not painted over by `compose`, so its value does not matter.
pub fn render((w, h): (i32, i32), m: i32, radius: f32, scale: f32) -> Vec<u32> {
    let (fw, fh) = (w + 2 * m, h + 2 * m);
    let (cx, cy) = (fw as f32 / 2.0, fh as f32 / 2.0 + OFFSET_Y * scale);
    // The shadow is shifted down, so it must be gone by the time it reaches the window's bottom edge, `m - offset` away from the shifted card.
    let reach = m as f32 - OFFSET_Y * scale;
    (0..fh * fw)
        .map(|i| {
            let (x, y) = ((i % fw) as f32 + 0.5, (i / fw) as f32 + 0.5);
            let d = distance(x, y, cx, cy, w as f32 / 2.0, h as f32 / 2.0, radius);
            let fade = (1.0 - d / reach).clamp(0.0, 1.0);
            ((STRENGTH * fade * fade * 255.0).round() as u32) << 24
        })
        .collect()
}

/// Puts the card over its shadow: for every card pixel, `card + shadow * (1 - card alpha)`, premultiplied, which is what the layered window blends. Input: the frame to fill, the shadow, the card pixels, the card width and the margin.
pub fn compose(frame: &mut [u32], shadow: &[u32], card: &[u32], cw: i32, m: i32) {
    frame.copy_from_slice(shadow);
    let fw = cw + 2 * m;
    for (i, &c) in card.iter().enumerate() {
        let (x, y) = (i as i32 % cw + m, i as i32 / cw + m);
        let at = (y * fw + x) as usize;
        let keep = 255 - (c >> 24);
        let under = frame[at];
        let ch = |shift: u32| ((((c >> shift) & 0xFF) + ((under >> shift) & 0xFF) * keep / 255).min(255)) << shift;
        frame[at] = ch(24) | ch(16) | ch(8) | ch(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shadow is darkest next to the card, weaker further out, gone at the outer edge, and heavier below the card than above it.
    #[test]
    fn fades_with_distance_and_sits_lower() {
        let (w, h, m) = (100, 60, 14);
        let px = render((w, h), m, 8.0, 1.0);
        let fw = (w + 2 * m) as usize;
        let alpha = |x: usize, y: usize| px[y * fw + x] >> 24;
        let mid = fw / 2;
        let (below_edge, below_far) = (alpha(mid, (m + h) as usize + 1), alpha(mid, (m + h) as usize + 12));
        assert!(below_edge > below_far && below_far < below_edge / 2, "{below_edge} {below_far}");
        assert_eq!(alpha(mid, (m + h) as usize + 13), 0, "nothing at the window edge");
        assert!(alpha(mid, (m + h) as usize + 4) > alpha(mid, (m as usize).saturating_sub(5)), "heavier below than above");
    }

    /// An opaque card pixel hides the shadow, a transparent one shows it, and a half-transparent one blends.
    #[test]
    fn card_goes_over_the_shadow() {
        let m = 1;
        let shadow = vec![0x4000_0000; 9];
        let mut frame = vec![0; 9];
        compose(&mut frame, &shadow, &[0xFF10_2030], 1, m);
        assert_eq!(frame[4], 0xFF10_2030);
        assert_eq!(frame[0], 0x4000_0000);
        compose(&mut frame, &shadow, &[0x0000_0000], 1, m);
        assert_eq!(frame[4], 0x4000_0000);
        compose(&mut frame, &shadow, &[0x8000_0000], 1, m);
        assert!(frame[4] >> 24 > 0x80 && frame[4] >> 24 < 0xFF);
    }
}
