//! File: shape.rs
//! Purpose: Anti-aliasing math shared by the software-drawn windows (recording overlay, tray menu).
//! Contents: distance — signed distance to a rounded rectangle; coverage — that distance turned into 0..1 pixel coverage.

/// Signed distance (px, negative inside) from (px, py) to a rounded rectangle centred at (cx, cy) with half-size (hw, hh) and radius r.
pub fn distance(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let (qx, qy) = ((px - cx).abs() - hw + r, (py - cy).abs() - hh + r);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

/// Anti-aliased coverage (0..1) of a rounded rectangle centred at (cx, cy) with half-size (hw, hh) and radius r.
pub fn coverage(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    (0.5 - distance(px, py, cx, cy, hw, hh, r)).clamp(0.0, 1.0)
}
