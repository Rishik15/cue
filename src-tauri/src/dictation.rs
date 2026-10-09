//! Purpose: Dictation flow glue: hotkey -> mic -> overlay -> (speech engine, M2) -> insertion.
//! Contents: init — start the audio and overlay threads; on_hotkey — Hold / Toggle state machine, never blocks;
//! start_from_menu — begin dictation from the tray menu. A 50 ms release grace and an 80 ms bounce guard follow Handy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::platform::{self, overlay, InsertOptions};
use crate::prefs::Prefs;
use crate::{audio, settings, status};

/// Recording runs this long after the key is released so the last syllable is not clipped.
const RELEASE_GRACE: Duration = Duration::from_millis(50);
/// A press this soon after a stop is switch bounce (or would collide with the grace period) and is ignored.
const BOUNCE: Duration = Duration::from_millis(80);

static LAST_STOP: Mutex<Option<Instant>> = Mutex::new(None);
static RECORDING: AtomicBool = AtomicBool::new(false);
/// Dictation started from the tray menu has no key held, so the next press of the hotkey stops it, in either mode.
static FROM_MENU: AtomicBool = AtomicBool::new(false);

pub fn init(app: &AppHandle) -> std::io::Result<()> {
    let handle = app.clone();
    audio::init(Arc::new(move |problem| status::report(&handle, &problem)))?;
    overlay::init()
}

/// `down` is the hotkey state. Hold: record while held. Toggle: each press flips recording.
pub fn on_hotkey(app: &AppHandle, down: bool) {
    let toggle = FROM_MENU.load(Ordering::Acquire) || settings::get_str(app, "dictation_mode").as_deref() == Some("Toggle");
    let recording = RECORDING.load(Ordering::Acquire);
    match (toggle, down, recording) {
        (_, true, false) if !bounced() => start(app),
        (false, false, true) | (true, true, true) => stop(app),
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
    RECORDING.store(true, Ordering::Release);
    audio::start(audio::Config { mic: prefs.mic, max_secs: prefs.max_record_secs, linger: Duration::from_secs(prefs.mic_linger_secs) });
    if prefs.show_overlay {
        overlay::show(prefs.overlay_top);
    }
}

fn stop(app: &AppHandle) {
    FROM_MENU.store(false, Ordering::Release);
    RECORDING.store(false, Ordering::Release);
    *LAST_STOP.lock().unwrap() = Some(Instant::now());
    overlay::hide();
    let app = app.clone();
    let min_samples = (audio::RATE as u64 * Prefs::load(&app).min_record_ms / 1000) as usize; // shorter is an accidental tap
    std::thread::spawn(move || {
        std::thread::sleep(RELEASE_GRACE);
        audio::stop(move |samples| {
            if samples.len() >= min_samples.max(1) {
                transcribe_and_insert(&app, &samples);
            }
        });
    });
}

/// No speech engine until M2: debug builds insert a capture summary so the whole path can be tried.
fn transcribe_and_insert(app: &AppHandle, samples: &[f32]) {
    if !cfg!(debug_assertions) {
        return;
    }
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let text = format!("[cue: {:.1}s captured, peak {peak:.2}] ", samples.len() as f32 / audio::RATE as f32);
    let prefs = Prefs::load(app);
    let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = platform::insert(&text, &options) {
            status::report(&app, &e.to_string());
        }
    });
}
