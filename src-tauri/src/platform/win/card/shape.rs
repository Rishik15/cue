//! File: shape.rs
//! Purpose: Paint the body of a floating card as pixels: a rounded rectangle with an arrow that points at the selected text, a hairline border and a soft shadow, all anti-aliased and premultiplied for a per-pixel-alpha window. No OS calls, so it is testable.
//! Contents:
//! - Spec / Arrow — what to draw
//! - render — paints the pixels (premultiplied BGRA as `0xAARRGGBB`) into the caller's buffer and returns the alpha channel kept apart so text drawn afterwards cannot spoil it
//! - the helpers are signed-distance functions and a box blur

use crate::platform::place::Side;

/// The arrow of a card: the edge it is on, its place along that edge and its size in px.
pub struct Arrow {
    pub side: Side,
    /// Distance of the arrow's middle from the start of its edge, in px.
    pub offset: f32,
    pub half_width: f32,
    pub reach: f32,
}

/// What `render` needs to paint a card body: bitmap size, the card rectangle, corner radius, the arrow and the shadow.
pub struct Spec {
    pub width: usize,
    pub height: usize,
    /// The card: left, top, width, height in px.
    pub card: (f32, f32, f32, f32),
    pub radius: f32,
    pub arrow: Option<Arrow>,
    pub fill: [f32; 3],
    pub border: [f32; 3],
    /// Hairline width and shadow geometry in px.
    pub border_width: f32,
    pub shadow_dy: f32,
    pub shadow_blur: f32,
    pub shadow_alpha: f32,
}

/// How much of a pixel a shape covers at signed distance `d` from its edge: an eased ramp about one and a half pixels wide, so curves look smooth instead of stair-stepped.
fn coverage(d: f32) -> f32 {
    let t = clamp01(0.5 - d / 1.5);
    t * t * (3.0 - 2.0 * t)
}

/// Limits `v` to 0..1.
fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Signed distance from point (`px`, `py`) to a rounded box (left, top, width, height) of corner radius `r`; negative inside.
fn rounded_box(px: f32, py: f32, (left, top, w, h): (f32, f32, f32, f32), r: f32) -> f32 {
    let (cx, cy, hx, hy) = (left + w / 2.0, top + h / 2.0, w / 2.0 - r, h / 2.0 - r);
    let (qx, qy) = ((px - cx).abs() - hx, (py - cy).abs() - hy);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

/// Exact distance to the triangle `a`, `b`, `c` (negative inside).
fn triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    let edge = |from: (f32, f32), to: (f32, f32)| {
        let (e, d) = ((to.0 - from.0, to.1 - from.1), (p.0 - from.0, p.1 - from.1));
        let t = ((d.0 * e.0 + d.1 * e.1) / (e.0 * e.0 + e.1 * e.1)).clamp(0.0, 1.0);
        let (qx, qy) = (d.0 - e.0 * t, d.1 - e.1 * t);
        (qx * qx + qy * qy, d.0 * e.1 - d.1 * e.0)
    };
    let (e0, e1, e2) = (edge(a, b), edge(b, c), edge(c, a));
    let sign = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let sign = if sign < 0.0 { -1.0 } else { 1.0 };
    let (dist, side) = [e0, e1, e2].into_iter().fold((f32::MAX, f32::MIN), |(d, s), (de, se)| (d.min(de), s.max(se * sign)));
    dist.sqrt() * if side > 0.0 { 1.0 } else { -1.0 }
}

/// The arrow's signed distance: a triangle standing on the card's edge, its tip rounded. The base is buried in the card. Only the tip is rounded; where the arrow meets the card the corners stay sharp. `u` runs along the edge from the arrow's middle, `v` leaves the card.
fn arrow_distance(px: f32, py: f32, card: (f32, f32, f32, f32), arrow: &Arrow) -> f32 {
    let (left, top, w, h) = card;
    let (u, v) = match arrow.side {
        Side::Top => (px - (left + arrow.offset), top - py),
        Side::Bottom => (px - (left + arrow.offset), py - (top + h)),
        Side::Left => (py - (top + arrow.offset), left - px),
        Side::Right => (py - (top + arrow.offset), px - (left + w)),
    };
    let tip = (arrow.half_width * 0.25).max(1.5);
    let sink = 4.0;
    let slope = arrow.half_width / arrow.reach;
    let sin = slope / (1.0 + slope * slope).sqrt();
    let apex = arrow.reach - tip / sin;
    let base = (apex + sink) * slope;
    triangle((u, v), (-base, -sink), (base, -sink), (0.0, apex)) - tip
}

/// Box blur of `buf` (`w` by `h`) with the given radius, horizontally then vertically.
fn box_blur(buf: &mut [f32], w: usize, h: usize, radius: usize) {
    let r = radius as isize;
    let window = (2 * radius + 1) as f32;
    let mut line = vec![0.0f32; w.max(h)];
    for y in 0..h {
        let row = &mut buf[y * w..(y + 1) * w];
        blur_line(row, &mut line[..w], r, window);
    }
    for x in 0..w {
        let mut column: Vec<f32> = (0..h).map(|y| buf[y * w + x]).collect();
        blur_line(&mut column, &mut line[..h], r, window);
        (0..h).for_each(|y| buf[y * w + x] = column[y]);
    }
}

/// Box-blurs one line of `data` with radius `r` and a window of `window` samples, using `scratch` as working space.
fn blur_line(data: &mut [f32], scratch: &mut [f32], r: isize, window: f32) {
    let n = data.len() as isize;
    let at = |i: isize| if i < 0 || i >= n { 0.0 } else { data[i as usize] };
    let mut sum: f32 = (-r..=r).map(at).sum();
    for i in 0..n {
        scratch[i as usize] = sum / window;
        sum += at(i + r + 1) - at(i - r);
    }
    data.copy_from_slice(scratch);
}

/// The shadow is smooth, so it is worked out on a grid this many times coarser than the pixels and stretched back.
const SHADOW_GRID: usize = 4;

/// Linear interpolation into a coarse grid (`gw` by `gh`) at pixel (x, y) of the full-size image.
fn sample(grid: &[f32], gw: usize, gh: usize, x: usize, y: usize) -> f32 {
    let at = |v: usize| (v as f32 + 0.5) / SHADOW_GRID as f32 - 0.5;
    let (fx, fy) = (at(x).max(0.0), at(y).max(0.0));
    let (x0, y0) = ((fx as usize).min(gw - 1), (fy as usize).min(gh - 1));
    let (x1, y1) = ((x0 + 1).min(gw - 1), (y0 + 1).min(gh - 1));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let top = grid[y0 * gw + x0] * (1.0 - tx) + grid[y0 * gw + x1] * tx;
    let bottom = grid[y1 * gw + x0] * (1.0 - tx) + grid[y1 * gw + x1] * tx;
    top * (1.0 - ty) + bottom * ty
}

/// The blurred, offset silhouette of the card on a coarse grid; returns it with the grid's size.
fn shadow_grid(spec: &Spec, distance: &dyn Fn(f32, f32) -> f32) -> (Vec<f32>, usize, usize) {
    let (gw, gh) = (spec.width / SHADOW_GRID + 2, spec.height / SHADOW_GRID + 2);
    let mut shadow = vec![0.0f32; gw * gh];
    let half = SHADOW_GRID as f32 / 2.0;
    for j in 0..gh {
        for i in 0..gw {
            let (px, py) = ((i * SHADOW_GRID) as f32 + half, (j * SHADOW_GRID) as f32 + half - spec.shadow_dy);
            shadow[j * gw + i] = clamp01(0.5 - distance(px, py));
        }
    }
    let radius = ((spec.shadow_blur / 2.0) / SHADOW_GRID as f32).round().max(1.0) as usize;
    for _ in 0..2 {
        box_blur(&mut shadow, gw, gh, radius);
    }
    (shadow, gw, gh)
}

/// Paints the card into `pixels` (w * h) and returns its alpha channel separately. Deep inside the card and far outside it nothing is measured: only a band around the edge needs the exact distance. Whole rows of the card's inside are filled at once; only the band around the edge and the shadow outside are worked out pixel by pixel.
pub fn render(spec: &Spec, pixels: &mut [u32]) -> Vec<u8> {
    let (w, h) = (spec.width, spec.height);
    let (left, top, cw, ch) = spec.card;
    let distance = |px: f32, py: f32| {
        let body = rounded_box(px, py, spec.card, spec.radius);
        spec.arrow.as_ref().map_or(body, |a| body.min(arrow_distance(px, py, spec.card, a)))
    };
    let deep = spec.radius + spec.border_width + 2.0;
    let reach = spec.arrow.as_ref().map_or(0.0, |a| a.reach + a.half_width) + 6.0;
    let (shadow, gw, gh) = shadow_grid(spec, &distance);
    let mut alpha = vec![0u8; w * h];
    let solid = {
        let channel = |c: usize| (clamp01(spec.fill[c]) * 255.0).round() as u32;
        0xff00_0000 | channel(0) << 16 | channel(1) << 8 | channel(2)
    };
    let (x_from, x_to) = (((left + deep).ceil() as usize).min(w), ((left + cw - deep).floor().max(0.0) as usize).min(w));
    for y in 0..h {
        let row_inside = (y as f32 + 0.5) > top + deep && (y as f32 + 0.5) < top + ch - deep && x_from < x_to;
        let skip = if row_inside { x_from..x_to } else { 0..0 };
        for x in (0..w).filter(|x| !skip.contains(x)) {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (cov, inner) = if px < left - reach || px > left + cw + reach || py < top - reach || py > top + ch + reach {
                (0.0, 0.0)
            } else {
                let d = distance(px, py);
                (coverage(d), coverage(d + spec.border_width))
            };
            let shade = sample(&shadow, gw, gh, x, y) * spec.shadow_alpha * (1.0 - cov);
            let channel = |c: usize| (clamp01(spec.fill[c] * inner + spec.border[c] * (cov - inner)) * 255.0).round() as u32;
            let alpha8 = (clamp01(cov + shade) * 255.0).round() as u32;
            pixels[y * w + x] = alpha8 << 24 | channel(0) << 16 | channel(1) << 8 | channel(2);
            alpha[y * w + x] = alpha8 as u8;
        }
        if row_inside {
            pixels[y * w + x_from..y * w + x_to].fill(solid);
            alpha[y * w + x_from..y * w + x_to].fill(255);
        }
    }
    alpha
}
