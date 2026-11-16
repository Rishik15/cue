//! File: dictation/mod.rs
//! Purpose: Dictation flow glue: hotkey -> mic -> overlay -> speech engine -> insertion.
//! Contents: init — start the audio and overlay threads; on_hotkey — hotkey to start / stop via `mode`, never blocks;
//! cancel — Escape discards a recording in progress (never text already spoken). A release waits out a short regrip window before it stops, so a shift of finger pressure does not end the recording.
//! The text of a finished dictation is typed at the cursor or, when that cannot be done, left on the clipboard: it is never dropped.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod clean;
mod context;
mod deliver;
mod mode;
pub mod mute;
mod recording;
pub mod recovery;
mod routing;

use tauri::AppHandle;

use mode::{confirm_stop, decide, Action, Mode};
use recording::RECORDING;
use routing::{dispatch, route, Live};

use crate::platform::{self, overlay, Target};
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
/// The speech id whose engine failed (0: none). Its stop is final: a regrip or a release cannot cancel it, and a failure that arrives before the id is published is acted on when it is.
static FAILED_ID: AtomicU64 = AtomicU64::new(0);
static STOPS: AtomicU64 = AtomicU64::new(0);
/// The stop request (from a release) still waiting out the regrip window, or 0. A press meanwhile is the key coming back, not a new press.
static REGRIP_OF: AtomicU64 = AtomicU64::new(0);
/// The settings the live caption uses, read once when the dictation starts instead of from disk for every sentence.
static CAPTION_PREFS: Mutex<Option<Arc<Prefs>>> = Mutex::new(None);

/// What is known about one finished recording: its length for the statistics and the window it was dictated into. Taken when the user
/// stops, so a dictation started right after cannot change them.
#[derive(Clone)]
struct Spoken {
    ms: u64,
    target: Target,
}

/// Whether a recording is in progress. Returns true from the hotkey press until its stop is taken.
pub fn is_recording() -> bool {
    RECORDING.is_recording()
}

/// Starts the audio and overlay threads and registers the speech callbacks. Input: the app. Returns why a thread could not start.
pub fn init(app: &AppHandle) -> std::io::Result<()> {
    let handle = app.clone();
    audio::init(Arc::new(move |problem| status::report(&handle, &problem)))?;
    speech::on_partial(show_caption);
    let handle = app.clone();
    speech::on_failed(move |id| engine_failed(&handle, id));
    mute::init(app);
    overlay::init()
}

/// A sentence was recognised: show it in the pill, cleaned the same way as the text that will be typed.
fn show_caption(id: u64, index: usize, text: &str) {
    if id != RECORDING.id() {
        return; // belongs to an earlier dictation
    }
    let Some(prefs) = CAPTION_PREFS.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return };
    let shown = clean::clean(text, &prefs.vocabulary, prefs.remove_fillers);
    if prefs.show_overlay && !shown.is_empty() {
        overlay::caption(index, &shown);
    }
}

/// The engine failed or was unloaded while recording `id`: end the recording as a normal stop, so the microphone and the mute are released at once and
/// what was recognised is rescued, instead of the user talking on into a recording nothing listens to. Inputs: the app and the dictation id.
fn engine_failed(app: &AppHandle, id: u64) {
    FAILED_ID.store(id, Ordering::Release);
    if RECORDING.id() == id && RECORDING.is_recording() {
        stop(app, false, id);
    }
}

/// `down` is the hotkey state; `mode::decide` says what it means in the user's dictation mode.
pub fn on_hotkey(app: &AppHandle, down: bool) {
    let mode = current_mode();
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
    match decide(mode, down, RECORDING.is_recording(), held) {
        Action::Start if !bounced() => start(app),
        Action::Stop => stop(app, !down, RECORDING.id()),
        _ => {}
    }
}

/// The dictation mode from settings.
fn current_mode() -> Mode {
    Mode::parse(settings::get_str("dictation_mode").as_deref())
}

/// Whether the last stop was so recent that a press now is key bounce.
fn bounced() -> bool {
    LAST_STOP.lock().unwrap().is_some_and(|t| t.elapsed() < BOUNCE)
}

/// Starts a dictation: the speech model begins loading, the microphone opens and the overlay shows. Input: the app.
fn start(app: &AppHandle) {
    if crate::snippets::ask::is_open() {
        return; // the question window has the keyboard
    }
    crate::dictionary::cancel(); // a lookup or card gives way to a dictation
    status::clear(app);
    let prefs = Arc::new(Prefs::load());
    if !speech::has_model(app) {
        // Nothing could be transcribed: say so now instead of recording, loading and then failing.
        status::report_as(app, "No speech model. Download one in Settings, Models.", "Download a model to start dictating");
        return;
    }
    if !RECORDING.begin(platform::capture_target) {
        return; // the last recording is still being ended: this press is dropped, not started under its late stop
    }
    let id = speech::begin(app); // the model loads while the user speaks
    if let Some(task) = rules::model_task_for(Trigger::Dictation) {
        crate::text::prepare(task); // and so does the text model, if a rule will use it
    }
    RECORDING.publish(id);
    *CAPTION_PREFS.lock().unwrap_or_else(|e| e.into_inner()) = Some(prefs.clone());
    mute::start();
    if prefs.show_overlay {
        overlay::show(id, prefs.overlay_top, prefs.theme());
    }
    let sink = Arc::new(move |chunk| speech::feed(id, chunk)); // speech is recognised during the recording, not after it
    audio::start(audio::Config {
        mic: prefs.mic.clone(),
        max_secs: prefs.max_record_secs,
        linger: Duration::from_secs(prefs.mic_linger_secs),
        sink,
        on_cap: capped(app, id),
        on_fail: Arc::new(move || cancel_recording(id)), // no microphone: do not leave the overlay up and the speakers muted
    });
    if FAILED_ID.load(Ordering::Acquire) == id {
        stop(app, false, id); // the engine failed before this recording had published its id
    }
}

/// The recording reached its maximum length: say so and end it as a normal stop, so what was said is still typed.
fn capped(app: &AppHandle, id: u64) -> Arc<dyn Fn() + Send + Sync> {
    let app = app.clone();
    Arc::new(move || {
        if RECORDING.id() != id {
            return; // a newer dictation owns the recording now
        }
        status::report_as(&app, "Reached the recording limit. Everything said so far is being typed.", "Recording limit reached");
        stop(&app, false, id);
    })
}

/// Cancels recording `id` only while it is still the current one, checked and ended under the phase lock: a late failure of an abandoned recording must not end the next one. Input: the dictation id.
fn cancel_recording(id: u64) {
    abandon(RECORDING.end_for(id));
}

/// Ends recording `id` after the regrip window, and only that recording: the id comes from the caller, never read again here, so a callback of an earlier recording cannot end the next one.
/// `from_release`: the stop came from letting go of the key, so it waits to see whether the key comes straight back.
fn stop(app: &AppHandle, from_release: bool, id: u64) {
    let ticket = STOPS.fetch_add(1, Ordering::AcqRel) + 1;
    if from_release {
        REGRIP_OF.store(ticket, Ordering::Release);
    }
    let app = app.clone();
    // The overlay stays up, grey and still, while the last words are recognised: it shows Cue heard you and is working.
    let spawned = crate::guard::spawn("dictation-stop", 128 * 1024, move || {
        std::thread::sleep(REGRIP);
        let _ = REGRIP_OF.compare_exchange(ticket, 0, Ordering::AcqRel, Ordering::Acquire);
        let superseded = STOPS.load(Ordering::Acquire) != ticket;
        let forced = FAILED_ID.load(Ordering::Acquire) == id; // the engine failed: nothing is left to regrip for
        if !(forced || confirm_stop(from_release, KEY_DOWN.load(Ordering::Acquire), superseded)) {
            return; // regripped or a newer stop owns it
        }
        let Some((target, _stopping)) = RECORDING.end_for(id) else { return }; // Escape already ended it, or a newer dictation owns the recording
        mute::stop();
        let released = Instant::now();
        *LAST_STOP.lock().unwrap() = Some(released);
        let min_samples = (audio::RATE as u64 * Prefs::min_record_ms() / 1000) as usize; // shorter is an accidental tap
        audio::stop(move |tail, total| {
            let spoken = Spoken { ms: total as u64 * 1000 / audio::RATE as u64, target };
            let keep = total >= min_samples.max(1);
            if !keep {
                overlay::hide(id); // nothing to wait for
            }
            speech::feed(id, tail);
            transcribe_and_insert(&app, id, keep, spoken, released);
        });
    });
    if spawned.is_err() {
        REGRIP_OF.store(0, Ordering::Release);
        abandon(RECORDING.end_for(id)); // cannot finish this recording: the hold lasts through the cleanup, and a recording that is not ours is left alone
    }
}

/// Escape: while recording, throw the audio away. Once the user has stopped, what was said is never dropped: it is still recognised and delivered.
pub fn cancel() {
    abandon(RECORDING.end());
}

/// Throws away the recording `ended` handed over (its microphone, mute, text and overlay); does nothing when nothing was ended. Input: the result of ending the recording.
fn abandon(ended: Option<(Target, recording::Stopping<'_>)>) {
    let Some((_target, _stopping)) = ended else { return };
    mute::stop();
    *LAST_STOP.lock().unwrap() = Some(Instant::now());
    audio::stop(|_, _| {});
    speech::cancel();
    overlay::dismiss();
}

/// `keep` is false for an accidental tap: its text is thrown away.
/// `released`: when recording stopped, so the log can show how long the user waited for the text (no text is logged).
fn transcribe_and_insert(app: &AppHandle, id: u64, keep: bool, spoken: Spoken, released: Instant) {
    let handle = app.clone();
    speech::finish(id, move |result| {
        if keep {
            crate::log::note(&format!(
                "dictation: {} ms spoken, text after {} ms, {} threads",
                spoken.ms,
                released.elapsed().as_millis(),
                speech::process::threads()
            ));
        }
        overlay::hide(id); // ignored when the user already started the next dictation, which owns the overlay now
        dispatch(route(result, keep), Live { app: &handle, spoken });
    });
}

#[cfg(test)]
mod tests;
