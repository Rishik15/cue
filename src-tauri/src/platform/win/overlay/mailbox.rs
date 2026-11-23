//! File: mailbox.rs
//! Purpose: What other threads say to the pill and who owns it: the public overlay calls post a message to the overlay thread (made on first use by `host`) and leave their data here, stamped with the dictation that owns the pill, so the overlay thread takes only data that is still current.
//! Contents: init / show / hide / dismiss / caption / notice — the public calls; message ids; owner, theme, notice_text, take_captions — what the overlay thread reads.

use crate::lock::lock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use windows::Win32::UI::WindowsAndMessaging::WM_APP;

use super::super::host::Host;
use crate::platform::Theme;

pub(super) const SHOW: u32 = WM_APP + 1;
pub(super) const HIDE: u32 = WM_APP + 2;
pub(super) const NOTICE: u32 = WM_APP + 3;
pub(super) const CAPTION: u32 = WM_APP + 4;
pub(super) const DISMISS: u32 = WM_APP + 5;

pub(super) static HOST: Host = Host::new("pill", super::run);
/// The dictation the pill shows; a `hide` from an earlier one must not close the pill of the next.
static OWNER: AtomicU64 = AtomicU64::new(0);
static THEME: Mutex<Option<Theme>> = Mutex::new(None);
static NOTICE_TEXT: Mutex<String> = Mutex::new(String::new());
/// Captions waiting for the overlay thread, each stamped with the dictation that said it; a caption of an earlier dictation is dropped when taken.
static CAPTION_QUEUE: Mutex<Vec<(u64, usize, String)>> = Mutex::new(Vec::new());

/// The dictation that owns the pill now.
pub(super) fn owner() -> u64 {
    OWNER.load(Ordering::Acquire)
}

/// The theme of the latest `show` or `notice`, or the default when none came.
pub(super) fn theme() -> Theme {
    lock(&THEME).clone().unwrap_or_default()
}

/// The text of the latest `notice`.
pub(super) fn notice_text() -> String {
    lock(&NOTICE_TEXT).clone()
}

/// Takes the queued captions that belong to the dictation owning the pill now; those of an earlier dictation are dropped. Return: (sentence number, text) pairs in the order they came.
pub(super) fn take_captions() -> Vec<(usize, String)> {
    let queued = std::mem::take(&mut *lock(&CAPTION_QUEUE));
    let current = owner();
    queued.into_iter().filter(|(from, ..)| *from == current).map(|(_, index, text)| (index, text)).collect()
}

/// Nothing to start: the window and its thread are made by the first `show` or `notice` and end when the pill is hidden. Kept so the dictation setup need not change.
pub fn init() -> std::io::Result<()> {
    Ok(())
}

/// `id` is the dictation; `top` picks the screen edge; `theme` is the user's Cue theme, read from settings by the caller.
pub fn show(id: u64, top: bool, theme: Theme) {
    OWNER.store(id, Ordering::Release);
    *lock(&THEME) = Some(theme);
    lock(&CAPTION_QUEUE).clear();
    HOST.post(SHOW, top as usize, 0, true);
}

/// Dictation `id` is over: the pill goes as soon as its caption has finished typing, unless a newer dictation owns it now (checked here and again on the overlay thread, where an older hide can arrive after the next show).
pub fn hide(id: u64) {
    if OWNER.load(Ordering::Acquire) == id {
        HOST.post(HIDE, id as usize, 0, false);
    }
}

/// The pill goes now (Escape, or the speech engine was stopped).
pub fn dismiss() {
    HOST.post(DISMISS, 0, 0, false);
}

/// Sentence number `index` of the current dictation, for the live caption. Ignored when the overlay is not showing (the entry is queued before the message is posted so the overlay thread cannot miss it, and taken back when nothing was there to hear it), and dropped when a newer dictation has taken the pill before it is drawn.
pub fn caption(index: usize, text: &str) {
    lock(&CAPTION_QUEUE).push((OWNER.load(Ordering::Acquire), index, text.to_string()));
    if !HOST.post(CAPTION, 0, 0, false) {
        lock(&CAPTION_QUEUE).pop();
    }
}

/// Shows a short message in the overlay's place for a few seconds, then hides it.
pub fn notice(text: &str, top: bool, theme: Theme) {
    *lock(&THEME) = Some(theme);
    *lock(&NOTICE_TEXT) = text.to_string();
    HOST.post(NOTICE, top as usize, 0, true);
}
