//! Purpose: Dictation flow glue: hotkey -> mic -> overlay -> speech engine -> insertion.
//! Contents: init — start the audio and overlay threads; on_hotkey — hotkey to start / stop via `mode`, never blocks;
//! start_from_menu — begin dictation from the tray menu; cancel — Escape stops recording or drops text still being transcribed. A 50 ms release grace and an 80 ms bounce guard follow Handy.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod clean;
mod mode;

use tauri::AppHandle;

use mode::{decide, Action, Mode};

use crate::platform::{self, overlay, InsertOptions};
use crate::prefs::Prefs;
use crate::{audio, settings, speech, status};

/// Recording runs this long after the key is released so the last syllable is not clipped.
const RELEASE_GRACE: Duration = Duration::from_millis(50);
/// A press this soon after a stop is switch bounce (or would collide with the grace period) and is ignored.
const BOUNCE: Duration = Duration::from_millis(80);

static LAST_STOP: Mutex<Option<Instant>> = Mutex::new(None);
/// When the hotkey last went down; Hold or Toggle needs the press length on release.
static PRESSED_AT: Mutex<Option<Instant>> = Mutex::new(None);
static RECORDING: AtomicBool = AtomicBool::new(false);
/// The speech id of the recording in progress; `stop` needs it to finish the right one.
static SPEECH_ID: AtomicU64 = AtomicU64::new(0);
/// The window that had focus when dictation started. Text is only typed if it still has focus when the text is ready.
static TARGET: AtomicIsize = AtomicIsize::new(0);
/// Dictation started from the tray menu has no key held, so the next press of the hotkey stops it, in either mode.
static FROM_MENU: AtomicBool = AtomicBool::new(false);

pub fn init(app: &AppHandle) -> std::io::Result<()> {
    let handle = app.clone();
    audio::init(Arc::new(move |problem| status::report(&handle, &problem)))?;
    let handle = app.clone();
    speech::on_partial(move |id, index, text| show_caption(&handle, id, index, text));
    overlay::init()
}

/// A sentence was recognised: show it as the live caption, cleaned the same way as the text that will be typed.
fn show_caption(app: &AppHandle, id: u64, index: usize, text: &str) {
    if id != SPEECH_ID.load(Ordering::Acquire) {
        return; // belongs to an earlier dictation
    }
    let prefs = Prefs::load(app);
    let shown = clean::clean(text, &prefs.vocabulary, prefs.remove_fillers);
    if prefs.show_overlay && !shown.is_empty() {
        overlay::caption(index, &shown);
    }
}

/// `down` is the hotkey state; `mode::decide` says what it means in the user's dictation mode.
pub fn on_hotkey(app: &AppHandle, down: bool) {
    let mode = match FROM_MENU.load(Ordering::Acquire) {
        true => Mode::Toggle, // started from the menu: no key is held, so the next press ends it
        false => Mode::parse(settings::get_str(app, "dictation_mode").as_deref()),
    };
    let held = {
        let mut pressed = PRESSED_AT.lock().unwrap();
        if down {
            *pressed = Some(Instant::now());
        }
        pressed.map_or(Duration::ZERO, |t| t.elapsed())
    };
    match decide(mode, down, RECORDING.load(Ordering::Acquire), held) {
        Action::Start if !bounced() => start(app),
        Action::Stop => stop(app),
        _ => {}
    }
}

fn bounced() -> bool {
    LAST_STOP.lock().unwrap().is_some_and(|t| t.elapsed() < BOUNCE)
}

/// "Open Cue" in the tray menu: start listening now. The menu never takes focus, so the text lands in the app you were in.
pub fn start_from_menu(app: &AppHandle) {
    if !RECORDING.load(Ordering::Acquire) {
        FROM_MENU.store(true, Ordering::Release);
        start(app);
    }
}

fn start(app: &AppHandle) {
    status::clear(app);
    let prefs = Prefs::load(app);
    if !speech::has_model(app) {
        // Nothing could be transcribed: say so now instead of recording, loading and then failing.
        FROM_MENU.store(false, Ordering::Release);
        overlay::notice("Download a model to start dictating", prefs.overlay_top, prefs.theme());
        status::report(app, "No speech model. Download one in Settings, Models.");
        return;
    }
    RECORDING.store(true, Ordering::Release);
    TARGET.store(platform::foreground_window(), Ordering::Release);
    let theme = prefs.theme();
    let id = speech::begin(app); // the model loads while the user speaks
    SPEECH_ID.store(id, Ordering::Release);
    let sink = Arc::new(move |chunk| speech::feed(id, chunk)); // speech is recognised during the recording, not after it
    audio::start(audio::Config {
        mic: prefs.mic,
        max_secs: prefs.max_record_secs,
        linger: Duration::from_secs(prefs.mic_linger_secs),
        sink,
    });
    if prefs.show_overlay {
        overlay::show(prefs.overlay_top, theme);
    }
}

fn stop(app: &AppHandle) {
    FROM_MENU.store(false, Ordering::Release);
    RECORDING.store(false, Ordering::Release);
    *LAST_STOP.lock().unwrap() = Some(Instant::now());
    let (app, id) = (app.clone(), SPEECH_ID.load(Ordering::Acquire));
    // The overlay stays up, grey and still, while the last words are recognised: it shows Cue heard you and is working.
    let spawned = std::thread::Builder::new().name("dictation-stop".into()).stack_size(128 * 1024).spawn(move || {
        std::thread::sleep(RELEASE_GRACE);
        let min_samples = (audio::RATE as u64 * Prefs::load(&app).min_record_ms / 1000) as usize; // shorter is an accidental tap
        audio::stop(move |tail, total| {
            let keep = total >= min_samples.max(1);
            if !keep {
                overlay::hide(); // nothing to wait for
            }
            speech::feed(id, tail);
            transcribe_and_insert(&app, id, keep);
        });
    });
    if spawned.is_err() {
        overlay::hide(); // cannot finish this recording; do not leave the overlay up
    }
}

/// Escape: while recording, throw the audio away; afterwards, make sure nothing still being transcribed gets inserted.
pub fn cancel() {
    let was_recording = RECORDING.swap(false, Ordering::AcqRel);
    if was_recording {
        FROM_MENU.store(false, Ordering::Release);
        *LAST_STOP.lock().unwrap() = Some(Instant::now());
        audio::stop(|_, _| {});
    }
    if speech::cancel() || was_recording {
        overlay::dismiss();
    }
}

/// `keep` is false for an accidental tap: its text is thrown away.
fn transcribe_and_insert(app: &AppHandle, id: u64, keep: bool) {
    let handle = app.clone();
    speech::finish(id, move |result| {
        if !RECORDING.load(Ordering::Acquire) {
            overlay::hide(); // unless the user already started the next dictation, which owns the overlay now
        }
        match result {
            _ if !keep => {}
            Ok(text) => insert(&handle, text),
            Err(problem) => status::report(&handle, &problem),
        }
    });
}

/// Types the text where the cursor is, unless focus moved to another window since dictation started: then the text goes to the
/// clipboard and the user is told, so it never lands in the wrong app.
fn insert(app: &AppHandle, text: String) {
    let prefs = Prefs::load(app);
    let text = clean::clean(&text, &prefs.vocabulary, prefs.remove_fillers);
    if text.is_empty() {
        return; // silence, noise, or only filler sounds
    }
    let (target, now) = (TARGET.load(Ordering::Acquire), platform::foreground_window());
    if target != 0 && now != 0 && target != now {
        match platform::copy_text(&text) {
            Ok(()) => status::report(app, "You switched windows, so the text was copied to the clipboard. Paste it with Ctrl+V."),
            Err(e) => status::report(app, &format!("You switched windows and the text could not be copied: {e}")),
        }
        return;
    }
    let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
    let handle = app.clone();
    // Own thread: insertion can wait up to a second for modifier keys to be released, and must not hold up the speech thread.
    let spawned = std::thread::Builder::new().name("insert".into()).stack_size(512 * 1024).spawn(move || {
        if let Err(e) = platform::insert(&format!("{text} "), &options) {
            status::report(&handle, &e.to_string());
        }
    });
    if let Err(e) = spawned {
        status::report(app, &format!("Could not insert text: {e}"));
    }
}
