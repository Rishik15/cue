//! Purpose: Software drawing for the overlay: the DIB bitmap, the pill with its hairline border, and the level bars, all in the user's
//! theme colours. Text is drawn over it by `caption` / `notice` with GDI, which does not write alpha, so the opaque interior is re-stamped after.
//! Contents: Canvas — DIB section in a memory DC (BGRA premultiplied); Frame — what to paint this frame; paint — pill and bars (the bars
//! fade out when text takes over);
//! fade_left / opaque — the two passes that follow text drawing.

use std::cell::Cell;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::*;

use super::palette::{Palette, Rgb};
use crate::platform::win::shape::coverage;

pub const HEIGHT: f32 = 36.0; // dip
pub const BASE_WIDTH: f32 = 132.0;
const BARS: usize = 9;
const BAR_W: f32 = 3.0;
const BAR_GAP: f32 = 4.0;
const BARS_W: f32 = BARS as f32 * BAR_W + (BARS - 1) as f32 * BAR_GAP;
const FADE: Duration = Duration::from_millis(160);

/// A DIB section selected into a memory DC; `bits` is BGRA premultiplied.
pub struct Canvas {
    pub dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u32,
    pub w: i32,
    pub h: i32,
    pub scale: f32,
    since: Instant,
    /// When the microphone first delivered audio; the bars fade from idle to live from then on.
    live_at: Cell<Option<Instant>>,
}

pub struct Frame<'a> {
    /// Pixels of the pill this frame (the bitmap itself is allocated at the widest size).
    pub width: i32,
    pub level: f32,
    pub live: bool,
    /// 1 shows the bars in the middle of the pill, 0 hides them (a caption has taken over, or this is a message).
    pub bars_alpha: f32,
    pub palette: &'a Palette,
}

impl Canvas {
    pub unsafe fn new(w: i32, h: i32, scale: f32) -> Option<Self> {
        let dc = CreateCompatibleDC(None);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            return None;
        };
        SelectObject(dc, bitmap.into());
        Some(Canvas { dc, bitmap, bits: bits as *mut u32, w, h, scale, since: Instant::now(), live_at: Cell::new(None) })
    }

    fn pixels(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) }
    }

    /// 0 while the mic is not ready, easing to 1 over `FADE` once it is.
    fn wake(&self, live: bool) -> f32 {
        if !live {
            self.live_at.set(None);
            return 0.0;
        }
        let at = self.live_at.get().unwrap_or_else(|| {
            self.live_at.set(Some(Instant::now()));
            Instant::now()
        });
        (at.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.0)
    }

    /// Bar heights in px: flat until the mic is live, then following the voice with a little wave so they never freeze between syllables.
    fn bar_heights(&self, level: f32, wake: f32) -> [f32; BARS] {
        let (h, s) = (self.h as f32, self.scale);
        let t = self.since.elapsed().as_secs_f32();
        let amp = (level * 6.0).sqrt().min(1.0) * wake; // speech RMS is small; sqrt lifts quiet voices
        std::array::from_fn(|i| {
            let wave = 0.55 + 0.45 * (t * 9.0 + i as f32 * 0.9).sin();
            (3.0 * s + (h * 0.62 - 3.0 * s) * amp * wave).max(3.0 * s)
        })
    }

    /// The pill (opaque card colour, a hairline border, anti-aliased ends) with the level bars on it.
    pub fn paint(&mut self, f: &Frame) {
        let wake = self.wake(f.live);
        let heights = self.bar_heights(f.level, wake);
        let (w, h, s, p) = (f.width as f32, self.h as f32, self.scale, f.palette);
        let line = p.card.mix(p.ink, 0.16);
        let bar_ink = p.muted.mix(p.accent, wake);
        let x0 = (w - BARS_W * s) / 2.0;
        let (stride, rows) = (self.w, self.h);
        let px = self.pixels();
        for y in 0..rows {
            for x in 0..f.width {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let outer = coverage(fx, fy, w / 2.0, h / 2.0, w / 2.0, h / 2.0, h / 2.0);
                let inner = coverage(fx, fy, w / 2.0, h / 2.0, w / 2.0 - 1.0, h / 2.0 - 1.0, h / 2.0 - 1.0);
                let ring = (outer - inner).max(0.0);
                let mut c = (p.card.0 * inner + line.0 * ring, p.card.1 * inner + line.1 * ring, p.card.2 * inner + line.2 * ring);
                if f.bars_alpha > 0.0 {
                    c = blend_bars(c, fx, fy, (x0, bar_ink, &heights), (h, s, f.bars_alpha));
                }
                px[(y * stride + x) as usize] = pack(c, outer);
            }
        }
    }

    /// Text is drawn right-aligned and may run under the left edge of its area; this fades that edge into the card colour.
    pub fn fade_left(&mut self, area: &RECT, card: Rgb, width: i32) {
        let stride = self.w;
        let px = self.pixels();
        for x in area.left..(area.left + width).min(area.right) {
            let keep = (x - area.left) as f32 / width as f32;
            for y in area.top..area.bottom {
                let p = &mut px[(y * stride + x) as usize];
                let c = |shift: u32, target: f32| ((((*p >> shift) & 255) as f32 / 255.0) * keep + target * (1.0 - keep)).clamp(0.0, 1.0);
                *p = 0xFF00_0000 | to_byte(c(16, card.0)) << 16 | to_byte(c(8, card.1)) << 8 | to_byte(c(0, card.2));
            }
        }
    }

    /// GDI leaves the alpha byte of text pixels at 0; the area is inside the opaque card, so it is set back to 255.
    pub fn opaque(&mut self, area: &RECT) {
        let stride = self.w;
        let px = self.pixels();
        for y in area.top..area.bottom {
            for x in area.left..area.right {
                px[(y * stride + x) as usize] |= 0xFF00_0000;
            }
        }
    }
}

fn to_byte(v: f32) -> u32 {
    (v * 255.0).round() as u32
}

fn pack((r, g, b): (f32, f32, f32), a: f32) -> u32 {
    to_byte(a) << 24 | to_byte(r) << 16 | to_byte(g) << 8 | to_byte(b)
}

/// Lay the bars over a pill pixel `c` (premultiplied) at (`fx`, `fy`).
fn blend_bars(
    mut c: (f32, f32, f32),
    fx: f32,
    fy: f32,
    (x0, ink, heights): (f32, Rgb, &[f32; BARS]),
    (h, s, alpha): (f32, f32, f32),
) -> (f32, f32, f32) {
    let (bar_w, gap) = (BAR_W * s, BAR_GAP * s);
    for (i, bh) in heights.iter().enumerate() {
        let cx = x0 + i as f32 * (bar_w + gap) + bar_w / 2.0;
        if (fx - cx).abs() > bar_w {
            continue;
        }
        let k = coverage(fx, fy, cx, h / 2.0, bar_w / 2.0, bh / 2.0, bar_w / 2.0) * 0.95 * alpha;
        c = (ink.0 * k + c.0 * (1.0 - k), ink.1 * k + c.1 * (1.0 - k), ink.2 * k + c.2 * (1.0 - k));
    }
    c
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.bitmap.into());
        }
    }
}
