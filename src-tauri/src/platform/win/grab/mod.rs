//! Purpose: Grab Text's screen side: copy the whole desktop into memory, dim it, let the user drag a rectangle on it, and hand back the pixels inside.
//! Nothing stays resident: the picture, the window and the message loop live on the calling thread for the length of one grab, and every GDI object is freed on the way out.
//! Contents: grab_region — the one call; Shot — the desktop copy; wndproc — the drag; crop — the chosen pixels (enlarged, which the text reader needs).
//! The screen is copied before the selector window exists, so the selector never appears in the picture. Coordinates are physical pixels (the process is per-monitor DPI aware).

mod fixups;
pub mod ocr;
mod reflow;
mod select;
mod shot;

use windows::Win32::Foundation::RECT;

use crate::platform::Image;
use select::pick;
use shot::Shot;

/// Selections shorter than this many pixels are enlarged 3x, taller ones 2x: Windows' reader copes poorly with text under about 20 pixels high.
// ponytail: fixed factors, upgrade to a second read at the measured word height if small text still fails.
const SMALL: i32 = 60;

/// The user's rectangle of the screen as pixels; Ok(None) when they cancelled, Err when the screen could not be copied.
pub fn grab_region() -> Result<Option<Image>, String> {
    let shot = Shot::take().ok_or("Cue could not copy the screen to read it.")?;
    Ok(pick(&shot).and_then(|rect| crop(&shot, rect)))
}

/// The factor a selection is enlarged by before reading, kept so neither side passes the reader's size limit.
fn scale_for(w: i32, h: i32, limit: i32) -> i32 {
    let want = if h < SMALL { 3 } else { 2 };
    want.min((limit / w.max(h).max(1)).max(1))
}

fn crop(shot: &Shot, r: RECT) -> Option<Image> {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let k = scale_for(w, h, ocr::max_side());
    shot.copy_scaled(r, k).map(|bgra| Image { w: (w * k) as u32, h: (h * k) as u32, bgra })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_selections_are_enlarged_more_and_never_past_the_limit() {
        assert_eq!((scale_for(500, 20, 2600), scale_for(500, 400, 2600), scale_for(2000, 400, 2600)), (3, 2, 1));
    }
}
