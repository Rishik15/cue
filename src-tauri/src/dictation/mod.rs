//! Purpose: Dictation flow glue: hotkey -> mic -> overlay -> speech engine -> insertion.
//! Contents: init — start the audio and overlay threads; on_hotkey — hotkey to start / stop via `mode`, never blocks;
//! cancel — Escape discards a recording in progress (never text already spoken). A release waits out a short regrip window before it stops, so a shift of finger pressure does not end the recording.
//! The text of a finished dictation is typed at the cursor or, when that cannot be done, left on the clipboard: it is never dropped.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod clean;
#[cfg(test)]
mod clean_props;
mod context;
mod deliver;
mod mode;
mod mute;

use tauri::AppHandle;

use mode::{confirm_stop, decide, Action, Mode};

use crate::platform::{self, overlay};
use crate::prefs::Prefs;
use crate::rules::model::Trigger;
use crate::{audio, rules, settings, speech, status};

/// Recording runs this long after the key is released: the last syllable is not clipped, and a key that comes straight back down (a shift
/// of finger pressure on a laptop key) continues the same recording instead of ending it.
const REGRIP: Duration = Duration::from_millis(200);
/// A press that starts the next recording must not follow a stop by less than this (key bounce).
const BOUNCE: Duration = Duration::from_millis(80);

static LAST_STOP: Mutex<Option<Instant>> = Mutex::new(None);
/// When the hotkey last went down; Hold or Toggle needs the press length on release.
static PRESSED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Whether the hotkey is down now, and a count of stop requests: only the newest one may end the recording.
static KEY_DOWN: AtomicBool = AtomicBool::new(false);
static STOPS: AtomicU64 = AtomicU64::new(0);
/// The stop request (from a release) still waiting out the regrip window, or 0. A press meanwhile is the key coming back, not a new press.
static REGRIP_OF: AtomicU64 = AtomicU64::new(0);
static RECORDING: AtomicBool = AtomicBool::new(false);
/// The speech id of the recording in progress; `stop` needs it to finish the right one.
static SPEECH_ID: AtomicU64 = AtomicU64::new(0);
/// The window that had focus when dictation started. Text is only typed if it still has focus when the text is ready.
static TARGET: AtomicIsize = AtomicIsize::new(0);
/// The settings the live caption uses, read once when the dictation starts instead of from disk for every sentence.
static CAPTION_PREFS: Mutex<Option<Arc<Prefs>>> = Mutex::new(None);

/// What is known about one finished recording: its length for the statistics and the window it was dictated into. Taken when the user
/// stops, so a dictation started right after cannot change them.
#[derive(Clone, Copy)]
struct Spoken {
    ms: u64,
    target: isize,
}

pub fn is_recording() -> bool {
    RECORDING.load(Ordering::Acquire)
}

pub fn init(app: &AppHandle) -> std::io::Result<()> {
    let handle = app.clone();
    audio::init(Arc::new(move |problem| status::report(&handle, &problem)))?;
    speech::on_partial(show_caption);
    mute::init(app);
    overlay::init()
}

/// A sentence was recognised: show it in the pill, cleaned the same way as the text that will be typed.
fn show_caption(id: u64, index: usize, text: &str) {
    if id != SPEECH_ID.load(Ordering::Acquire) {
        return; // belongs to an earlier dictation
    }
    let Some(prefs) = CAPTION_PREFS.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return };
    let shown = clean::clean(text, &prefs.vocabulary, prefs.remove_fillers);
    if prefs.show_overlay && !shown.is_empty() {
        overlay::caption(index, &shown);
    }
}

/// `down` is the hotkey state; `mode::decide` says what it means in the user's dictation mode.
pub fn on_hotkey(app: &AppHandle, down: bool) {
    let mode = current_mode(app);
    KEY_DOWN.store(down, Ordering::Release);
    if down && REGRIP_OF.load(Ordering::Acquire) != 0 {
        return; // the key came straight back: recording simply continues
    }
    let held = {
        let mut pressed = PRESSED_AT.lock().unwrap();
        if down {
            *pressed = Some(Instant::now());
        }
        pressed.map_or(Duration::ZERO, |t| t.elapsed())
    };
    match decide(mode, down, RECORDING.load(Ordering::Acquire), held) {
        Action::Start if !bounced() => start(app),
        Action::Stop => stop(app, !down),
        _ => {}
    }
}

fn current_mode(app: &AppHandle) -> Mode {
    Mode::parse(settings::get_str(app, "dictation_mode").as_deref())
}

fn bounced() -> bool {
    LAST_STOP.lock().unwrap().is_some_and(|t| t.elapsed() < BOUNCE)
}

fn start(app: &AppHandle) {
    if crate::snippets::ask::is_open() {
        return; // the question window has the keyboard
    }
    crate::dictionary::cancel(); // a lookup or card gives way to a dictation
    status::clear(app);
    let prefs = Arc::new(Prefs::load(app));
    if !speech::has_model(app) {
        // Nothing could be transcribed: say so now instead of recording, loading and then failing.
        status::report_as(app, "No speech model. Download one in Settings, Models.", "Download a model to start dictating");
        return;
    }
    RECORDING.store(true, Ordering::Release);
    TARGET.store(platform::foreground_window(), Ordering::Release);
    let id = speech::begin(app); // the model loads while the user speaks
    if let Some(task) = rules::model_task_for(Trigger::Dictation) {
        crate::text::prepare(task); // and so does the text model, if a rule will use it
    }
    SPEECH_ID.store(id, Ordering::Release);
    *CAPTION_PREFS.lock().unwrap_or_else(|e| e.into_inner()) = Some(prefs.clone());
    mute::start();
    let sink = Arc::new(move |chunk| speech::feed(id, chunk)); // speech is recognised during the recording, not after it
    audio::start(audio::Config {
        mic: prefs.mic.clone(),
        max_secs: prefs.max_record_secs,
        linger: Duration::from_secs(prefs.mic_linger_secs),
        sink,
        on_cap: capped(app),
    });
    if prefs.show_overlay {
        overlay::show(id, prefs.overlay_top, prefs.theme());
    }
}

/// The recording reached its maximum length: say so and end it as a normal stop, so what was said is still typed.
fn capped(app: &AppHandle) -> Arc<dyn Fn() + Send + Sync> {
    let app = app.clone();
    Arc::new(move || {
        status::report_as(&app, "Reached the recording limit. Everything said so far is being typed.", "Recording limit reached");
        stop(&app, false);
    })
}

/// `from_release`: the stop came from letting go of the key, so it waits to see whether the key comes straight back.
fn stop(app: &AppHandle, from_release: bool) {
    let ticket = STOPS.fetch_add(1, Ordering::AcqRel) + 1;
    if from_release {
        REGRIP_OF.store(ticket, Ordering::Release);
    }
    let (app, id) = (app.clone(), SPEECH_ID.load(Ordering::Acquire));
    // The overlay stays up, grey and still, while the last words are recognised: it shows Cue heard you and is working.
    let spawned = crate::guard::spawn("dictation-stop", 128 * 1024, move || {
        std::thread::sleep(REGRIP);
        let _ = REGRIP_OF.compare_exchange(ticket, 0, Ordering::AcqRel, Ordering::Acquire);
        let superseded = STOPS.load(Ordering::Acquire) != ticket;
        let stale = SPEECH_ID.load(Ordering::Acquire) != id; // a newer dictation began meanwhile: this stop is not for it
        if stale || !confirm_stop(from_release, KEY_DOWN.load(Ordering::Acquire), superseded) || !RECORDING.swap(false, Ordering::AcqRel) {
            return; // regripped, a newer stop owns it, or Escape already ended it
        }
        let target = TARGET.load(Ordering::Acquire);
        mute::stop();
        *LAST_STOP.lock().unwrap() = Some(Instant::now());
        let min_samples = (audio::RATE as u64 * Prefs::load(&app).min_record_ms / 1000) as usize; // shorter is an accidental tap
        audio::stop(move |tail, total| {
            let spoken = Spoken { ms: total as u64 * 1000 / audio::RATE as u64, target };
            let keep = total >= min_samples.max(1);
            if !keep {
                overlay::hide(id); // nothing to wait for
            }
            speech::feed(id, tail);
            transcribe_and_insert(&app, id, keep, spoken);
        });
    });
    if spawned.is_err() {
        REGRIP_OF.store(0, Ordering::Release);
        RECORDING.store(false, Ordering::Release);
        mute::stop();
        audio::stop(|_, _| {}); // the microphone must not keep recording into a dictation nobody will finish
        speech::cancel();
        overlay::hide(id); // cannot finish this recording; do not leave the overlay up
    }
}

/// Escape: while recording, throw the audio away. Once the user has stopped, what was said is never dropped: it is still recognised and delivered.
pub fn cancel() {
    if !RECORDING.swap(false, Ordering::AcqRel) {
        return;
    }
    mute::stop();
    *LAST_STOP.lock().unwrap() = Some(Instant::now());
    audio::stop(|_, _| {});
    speech::cancel();
    overlay::dismiss();
}

/// `keep` is false for an accidental tap: its text is thrown away.
fn transcribe_and_insert(app: &AppHandle, id: u64, keep: bool, spoken: Spoken) {
    let handle = app.clone();
    speech::finish(id, move |result| {
        overlay::hide(id); // ignored when the user already started the next dictation, which owns the overlay now
        match result {
            _ if !keep => {} // an accidental tap: nothing was said
            Ok(text) => deliver::insert_off_thread(&handle, text, spoken),
            Err(problem) => status::report(&handle, &problem),
        }
    });
}
