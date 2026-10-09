//! Purpose: Anti-aliasing math shared by the software-drawn windows (recording overlay, tray menu).
//! Contents: coverage — rounded-rectangle signed distance turned into 0..1 pixel coverage.

/// Anti-aliased coverage (0..1) of a rounded rectangle centred at (cx, cy) with half-size (hw, hh) and radius r.
pub fn coverage(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let (qx, qy) = ((px - cx).abs() - hw + r, (py - cy).abs() - hh + r);
    let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::coverage;

    #[test]
    fn pill_edges() {
        assert_eq!(coverage(50.0, 18.0, 50.0, 18.0, 50.0, 18.0, 18.0), 1.0); // centre
        assert_eq!(coverage(0.0, 0.0, 50.0, 18.0, 50.0, 18.0, 18.0), 0.0); // outside the rounded corner
    }
}
