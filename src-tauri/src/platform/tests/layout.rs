//! File: platform/tests/layout.rs
//! Purpose: Protect window sizing: a window shrinks to the work area and never vanishes.
//! Contents: tests of `fit_logical`.

use crate::platform::place::fit_logical;
use crate::platform::Rect;

/// A rectangle in pixels. Inputs: left, top, right, bottom. Returns it.
fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
    Rect { left, top, right, bottom }
}

/// A window shrinks to the work area (in logical pixels, with a margin) and never grows or vanishes.
#[test]
fn windows_shrink_to_fit_the_work_area_and_never_vanish() {
    let area = rect(0, 0, 1920, 1040);
    assert_eq!(fit_logical((2000.0, 500.0), area, 1.5, 10.0), (1260.0, 500.0));
    let (w, h) = fit_logical((800.0, 900.0), area, 1.5, 10.0);
    assert_eq!(w, 800.0);
    assert!((h - 673.333).abs() < 0.01, "{h}");
    assert_eq!(fit_logical((300.0, 300.0), rect(0, 0, 10, 10), 1.0, 20.0), (1.0, 1.0));
}
