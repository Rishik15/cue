//! File: mod.rs
//! Purpose: Grab Text's screen side: copy the whole desktop into memory, dim it, let the user drag a rectangle on it, and hand back the pixels inside.
//! Nothing stays resident: the picture, the window and the message loop live on the calling thread for the length of one grab, and every GDI object is freed on the way out.
//! Contents: grab_region — the one call; Shot — the desktop copy; wndproc — the drag (its frame is the theme accent); crop / plan — the chosen pixels (enlarged, which the text reader needs) and the budget check that runs before any of them is allocated.
//! The screen is copied before the selector window exists, so the selector never appears in the picture. Coordinates are physical pixels (the process is per-monitor DPI aware).
//! The pixel budget is 256 MiB in total over the desktop copy, the crop and the two copies WinRT makes of it; a selection longer than the reader's side limit, or whose copies would not fit, is refused before it is copied, with a message.
//! shortcut: the selector needs the whole desktop picture to dim it, so a desktop picture alone over the budget is refused ("too large"), grab the chosen region directly without a picture when such a desktop must work.

mod fixups;
pub mod ocr;
mod reflow;
mod select;
mod shot;

use windows::Win32::Foundation::RECT;

use super::palette::resolve;
use super::theme::system_uses_dark_theme;
use crate::platform::{Image, Theme};
pub use select::close as close_selector;
use select::pick;
use shot::Shot;

/// Selections shorter than this many pixels are enlarged 3x, taller ones 2x: Windows' reader copes poorly with text under about 20 pixels high. shortcut: fixed factors, upgrade to a second read at the measured word height if small text still fails.
const SMALL: i32 = 60;
/// The most memory, in bytes, a grab may hold at once.
const BUDGET: usize = 256 << 20;
/// How many whole copies of the crop exist at the peak: the crop itself and the two WinRT makes (the buffer and the bitmap).
const CROP_COPIES: usize = 3;

/// The user's rectangle of the screen as pixels; Ok(None) when they cancelled, Err when the screen could not be copied or the selection is too large.
pub fn grab_region(theme: &Theme) -> Result<Option<Image>, String> {
    let frame = resolve(theme, system_uses_dark_theme()).accent;
    let shot = Shot::take(frame, BUDGET)?;
    pick(&shot).map(|rect| crop(&shot, rect)).transpose()
}

/// The bytes of a `w` by `h` BGRA picture; None when the count overflows.
fn bytes(w: i32, h: i32) -> Option<usize> {
    usize::try_from(w).ok()?.checked_mul(usize::try_from(h).ok()?)?.checked_mul(4)
}

/// The factor a selection of `w` by `h` is enlarged by before reading: 2 or 3, lowered until neither side passes the reader's `limit` and the desktop copy (`desktop` bytes) plus the crop copies fit the budget. Err, before anything is allocated, when even the plain size is too large.
fn plan(w: i32, h: i32, limit: i32, desktop: usize) -> Result<i32, String> {
    let too_large = || "This part of the screen is too large to read. Select a smaller area.".to_string();
    if w <= 0 || h <= 0 || w > limit || h > limit {
        return Err(too_large());
    }
    let fits = |k: i32| {
        bytes(w * k, h * k).and_then(|b| b.checked_mul(CROP_COPIES)).and_then(|b| b.checked_add(desktop)).is_some_and(|b| b <= BUDGET)
    };
    let wanted = if h < SMALL { 3 } else { 2 };
    let by_side = (limit / w.max(h)).max(1);
    (1..=wanted.min(by_side)).rev().find(|k| fits(*k)).ok_or_else(too_large)
}

/// The chosen part of the screen as an enlarged BGRA picture; Err, before anything is copied, when it is too large, or when it could not be copied.
fn crop(shot: &Shot, r: RECT) -> Result<Image, String> {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let k = plan(w, h, ocr::max_side(), bytes(shot.w, shot.h).unwrap_or(BUDGET))?;
    let bgra = shot.copy_scaled(r, k).ok_or("Cue could not copy the chosen part of the screen.")?;
    Ok(Image { w: (w * k) as u32, h: (h * k) as u32, bgra })
}

#[cfg(test)]
mod tests {
    use super::{plan, BUDGET};

    /// A small selection is enlarged 3x, a taller one 2x, and neither passes the reader's side limit.
    #[test]
    fn selections_are_enlarged_within_the_side_limit() {
        assert_eq!(plan(400, 40, 2600, 0), Ok(3));
        assert_eq!(plan(400, 200, 2600, 0), Ok(2));
        assert_eq!(plan(1500, 200, 2600, 0), Ok(1));
        assert_eq!(plan(2600, 100, 2600, 0), Ok(1));
    }

    /// A selection over the side limit is refused before anything is copied.
    #[test]
    fn a_selection_over_the_side_limit_is_refused_up_front() {
        assert!(plan(2601, 100, 2600, 0).is_err());
        assert!(plan(100, 5000, 2600, 0).is_err());
        assert!(plan(0, 10, 2600, 0).is_err());
    }

    /// The desktop copy counts against the budget, so a big desktop lowers the factor and then refuses.
    #[test]
    fn the_desktop_copy_counts_against_the_budget() {
        let side = 2600;
        let all_but_a_little = BUDGET - 2600 * 2600 * 4 * 3 - 1;
        assert_eq!(plan(side, side, 2600, all_but_a_little), Ok(1));
        assert!(plan(side, side, 2600, all_but_a_little + 2).is_err());
        assert!(plan(500, 500, 2600, BUDGET).is_err());
    }
}
