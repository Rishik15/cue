//! Purpose: Software drawing for the recording overlay: DIB bitmap lifetime and the pill with level bars.
//! Contents: Canvas — DIB section in a memory DC (BGRA premultiplied) and its `paint`.

use std::time::Instant;

use windows::Win32::Graphics::Gdi::*;

use crate::platform::win::shape::coverage;

pub const WIDTH: f32 = 132.0; // logical px
pub const HEIGHT: f32 = 36.0;
const BARS: usize = 9;

/// A DIB section selected into a memory DC; `bits` is BGRA premultiplied.
pub struct Canvas {
    pub dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u32,
    pub w: i32,
    pub h: i32,
    scale: f32,
    since: Instant,
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
        Some(Canvas { dc, bitmap, bits: bits as *mut u32, w, h, scale, since: Instant::now() })
    }

    pub fn paint(&self, level: f32) {
        let px = unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) };
        let (w, h, s) = (self.w as f32, self.h as f32, self.scale);
        let t = self.since.elapsed().as_secs_f32();
        let amp = (level * 6.0).sqrt().min(1.0); // speech RMS is small; sqrt lifts quiet voices
        let (bar_w, gap) = (3.0 * s, 4.0 * s);
        let x0 = (w - (BARS as f32 * bar_w + (BARS - 1) as f32 * gap)) / 2.0;
        let heights: [f32; BARS] = std::array::from_fn(|i| {
            let wave = 0.55 + 0.45 * (t * 9.0 + i as f32 * 0.9).sin(); // keeps bars alive between syllables
            (3.0 * s + (h * 0.62 - 3.0 * s) * amp * wave).max(3.0 * s)
        });
        for y in 0..self.h {
            for x in 0..self.w {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let bg = coverage(fx, fy, w / 2.0, h / 2.0, w / 2.0, h / 2.0, h / 2.0) * 0.94;
                let (mut r, mut g, mut b, mut a) = (0.10 * bg, 0.10 * bg, 0.11 * bg, bg);
                for (i, bh) in heights.iter().enumerate() {
                    let cx = x0 + i as f32 * (bar_w + gap) + bar_w / 2.0;
                    if (fx - cx).abs() > bar_w {
                        continue;
                    }
                    let c = coverage(fx, fy, cx, h / 2.0, bar_w / 2.0, bh / 2.0, bar_w / 2.0) * 0.95;
                    (r, g, b, a) = (0.96 * c + r * (1.0 - c), 0.96 * c + g * (1.0 - c), 0.97 * c + b * (1.0 - c), c + a * (1.0 - c));
                }
                px[(y * self.w + x) as usize] =
                    ((a * 255.0) as u32) << 24 | ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32;
            }
        }
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.bitmap.into());
        }
    }
}
