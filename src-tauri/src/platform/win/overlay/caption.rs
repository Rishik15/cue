//! Purpose: The live caption: what you have said so far, typed out on one line as if someone were typing it. Older sentences sit dim,
//! the newest one is bright, and a caret follows the last typed character. How fast it types depends on how much is still waiting to be
//! shown: a short remainder types at a relaxed pace, a long one speeds up so the caption never falls far behind the speech.
//! Contents: Caption — sentences plus the typing cursor; set — add or replace a sentence; advance — one frame of typing;
//! visible_width / presence / typing_done — layout and animation inputs; draw — GDI text and caret for one frame.

use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;

use super::palette::Palette;

const FONT: f32 = 13.0; // dip
/// Typing speed in characters per second: a floor, plus this much for every character still waiting, up to a ceiling.
const MIN_CPS: f32 = 22.0;
const CPS_PER_WAITING: f32 = 6.0;
const MAX_CPS: f32 = 400.0;
/// How long the bars take to fade out once the first character is typed.
const HANDOVER: Duration = Duration::from_millis(160);
const BLINK: Duration = Duration::from_millis(530);
/// Space kept after the last character for the caret, in dip.
pub const CARET_ROOM: f32 = 5.0;

/// Characters per second for a caption with `waiting` characters still to show.
fn typing_rate(waiting: f32) -> f32 {
    (MIN_CPS + waiting * CPS_PER_WAITING).min(MAX_CPS)
}

pub struct Caption {
    font: HFONT,
    /// A memory DC with the font selected, used only to measure text.
    dc: HDC,
    line_height: i32,
    sentences: Vec<String>,
    /// All sentences on one line, and where the newest one starts (so older ones can be dimmer).
    chars: Vec<char>,
    newest_from: usize,
    /// Characters typed so far (fractional between frames).
    typed: f32,
    ticked: Instant,
    first_at: Option<Instant>,
}

impl Caption {
    pub unsafe fn new(scale: f32) -> Caption {
        let height = -((FONT * scale).round() as i32);
        let font = CreateFontW(
            height,
            0,
            0,
            0,
            500,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI Variable Text"),
        );
        let dc = CreateCompatibleDC(None);
        SelectObject(dc, font.into());
        let mut metrics = TEXTMETRICW::default();
        let _ = GetTextMetricsW(dc, &mut metrics);
        Caption {
            font,
            dc,
            line_height: metrics.tmHeight,
            sentences: Vec::new(),
            chars: Vec::new(),
            newest_from: 0,
            typed: 0.0,
            ticked: Instant::now(),
            first_at: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Sentence number `index` of this dictation. A repeat of one already shown (after a worker restart) replaces it quietly.
    pub fn set(&mut self, index: usize, text: &str) {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() || index > self.sentences.len() {
            return;
        }
        match index == self.sentences.len() {
            true => self.sentences.push(text),
            false => self.sentences[index] = text,
        }
        self.chars = self.sentences.join(" ").chars().collect();
        let newest = self.sentences.last().map_or(0, |s| s.chars().count());
        self.newest_from = self.chars.len() - newest;
        self.typed = self.typed.min(self.chars.len() as f32);
        self.first_at.get_or_insert_with(Instant::now);
    }

    /// Types for the time since the last frame. The more characters wait, the faster it goes.
    pub fn advance(&mut self) {
        let dt = self.ticked.elapsed().as_secs_f32();
        self.ticked = Instant::now();
        let waiting = self.chars.len() as f32 - self.typed;
        self.typed = (self.typed + typing_rate(waiting) * dt).min(self.chars.len() as f32);
    }

    pub fn typing_done(&self) -> bool {
        self.typed >= self.chars.len() as f32
    }

    fn visible(&self) -> usize {
        self.typed as usize
    }

    unsafe fn width_of(&self, chars: &[char]) -> i32 {
        let wide: Vec<u16> = chars.iter().collect::<String>().encode_utf16().collect();
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(self.dc, &wide, &mut size);
        size.cx
    }

    /// Width in px of what has been typed so far.
    pub unsafe fn visible_width(&self) -> i32 {
        self.width_of(&self.chars[..self.visible()])
    }

    /// 0 before the first character, easing to 1 as the bars give way to the text.
    pub fn presence(&self) -> f32 {
        let t = self.first_at.map_or(0.0, |at| (at.elapsed().as_secs_f32() / HANDOVER.as_secs_f32()).min(1.0));
        t * t * (3.0 - 2.0 * t)
    }

    /// Draws the typed text and the caret into `area`: centred while it fits, otherwise right-aligned so the newest characters stay in view
    /// (and the older ones run under the left edge, which the caller fades). Returns whether it overflowed.
    pub unsafe fn draw(&self, dc: HDC, area: &RECT, p: &Palette, scale: f32) -> bool {
        let shown = self.visible();
        let split = self.newest_from.min(shown);
        let (old, new) = (&self.chars[..split], &self.chars[split..shown]);
        let (old_w, new_w) = (self.width_of(old), self.width_of(new));
        let total = old_w + new_w;
        let room = area.right - area.left - (CARET_ROOM * scale) as i32;
        let x = if total <= room { area.left + (room - total) / 2 } else { area.right - total };
        let y = area.top + (area.bottom - area.top - self.line_height) / 2;

        let saved = SaveDC(dc);
        IntersectClipRect(dc, area.left, area.top, area.right, area.bottom);
        SelectObject(dc, self.font.into());
        SetBkMode(dc, TRANSPARENT);
        for (text, from, color) in [(old, x, p.muted), (new, x + old_w, p.ink)] {
            let wide: Vec<u16> = text.iter().collect::<String>().encode_utf16().collect();
            SetTextColor(dc, COLORREF(color.colorref()));
            let _ = TextOutW(dc, from, y, &wide);
        }
        self.draw_caret(dc, x + total, y, p, scale);
        let _ = RestoreDC(dc, saved);
        total > room // the caller fades the left edge
    }

    /// A thin accent bar after the last character: steady while typing, blinking when caught up.
    unsafe fn draw_caret(&self, dc: HDC, x: i32, y: i32, p: &Palette, scale: f32) {
        let on = !self.typing_done() || self.first_at.is_some_and(|at| (at.elapsed().as_millis() / BLINK.as_millis()).is_multiple_of(2));
        if !on {
            return;
        }
        let width = (1.5 * scale).round().max(1.0) as i32;
        let brush = CreateSolidBrush(COLORREF(p.accent.colorref()));
        FillRect(dc, &RECT { left: x + 1, top: y + 1, right: x + 1 + width, bottom: y + self.line_height - 1 }, brush);
        let _ = DeleteObject(brush.into());
    }
}

impl Drop for Caption {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.font.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_speeds_up_with_the_backlog_within_limits() {
        assert_eq!(typing_rate(0.0), MIN_CPS); // nothing waiting: relaxed pace
        assert!(typing_rate(10.0) > typing_rate(2.0));
        assert!(typing_rate(40.0) > typing_rate(10.0));
        assert_eq!(typing_rate(10_000.0), MAX_CPS);
        // A 100 character sentence is fully typed in well under a second; a short remainder takes its time.
        let (mut typed, mut seconds) = (0.0f32, 0.0f32);
        while typed < 100.0 {
            typed += typing_rate(100.0 - typed) / 60.0;
            seconds += 1.0 / 60.0;
        }
        assert!((0.3..0.7).contains(&seconds), "{seconds}");
    }
}
