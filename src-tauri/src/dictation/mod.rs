//! Purpose: Dictation flow glue: hotkey -> mic -> overlay -> speech engine -> insertion.
//! Contents: init — start the audio and overlay threads; on_hotkey — hotkey to start / stop via `mode`, never blocks;
//! start_from_menu — begin dictation from the tray menu; cancel — Escape stops recording or drops text still being transcribed. A release waits out a short regrip window before it stops, so a shift of finger pressure does not end the recording.
//! The text of a finished dictation is typed at the cursor or, when that cannot be done, left on the clipboard: it is never dropped.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod clean;
mod context;
mod mode;
mod mute;

use tauri::AppHandle;

use mode::{confirm_stop, decide, Action, Mode};

use crate::platform::{self, overlay, InsertOptions};
use crate::prefs::Prefs;
use crate::rules::model::{Output, Trigger};
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
/// Dictation started from the tray menu has no key held, so the next press of the hotkey stops it, in either mode.
static FROM_MENU: AtomicBool = AtomicBool::new(false);
/// How long the last recording was, in milliseconds, for the statistics.
static LAST_MS: AtomicU64 = AtomicU64::new(0);

pub fn is_recording() -> bool {
    RECORDING.load(Ordering::Acquire)
}

pub fn init(app: &AppHandle) -> std::io::Result<()> {
    let handle = app.clone();
    audio::init(Arc::new(move |problem| status::report(&handle, &problem)))?;
    let handle = app.clone();
    speech::on_partial(move |id, index, text| show_caption(&handle, id, index, text));
    mute::init(app);
    overlay::init()
}

/// A sentence was recognised: show it in the pill, cleaned the same way as the text that will be typed.
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
    match FROM_MENU.load(Ordering::Acquire) {
        true => Mode::Toggle, // started from the menu: no key is held, so the next press ends it
        false => Mode::parse(settings::get_str(app, "dictation_mode").as_deref()),
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
        overlay::show(prefs.overlay_top, prefs.theme());
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
    let spawned = std::thread::Builder::new().name("dictation-stop".into()).stack_size(128 * 1024).spawn(move || {
        std::thread::sleep(REGRIP);
        let _ = REGRIP_OF.compare_exchange(ticket, 0, Ordering::AcqRel, Ordering::Acquire);
        let superseded = STOPS.load(Ordering::Acquire) != ticket;
        if !confirm_stop(from_release, KEY_DOWN.load(Ordering::Acquire), superseded) || !RECORDING.swap(false, Ordering::AcqRel) {
            return; // regripped, a newer stop owns it, or Escape already ended it
        }
        FROM_MENU.store(false, Ordering::Release);
        mute::stop();
        *LAST_STOP.lock().unwrap() = Some(Instant::now());
        let min_samples = (audio::RATE as u64 * Prefs::load(&app).min_record_ms / 1000) as usize; // shorter is an accidental tap
        audio::stop(move |tail, total| {
            LAST_MS.store(total as u64 * 1000 / audio::RATE as u64, Ordering::Release);
            let keep = total >= min_samples.max(1);
            if !keep {
                overlay::hide(); // nothing to wait for
            }
            speech::feed(id, tail);
            transcribe_and_insert(&app, id, keep);
        });
    });
    if spawned.is_err() {
        REGRIP_OF.store(0, Ordering::Release);
        RECORDING.store(false, Ordering::Release);
        mute::stop();
        overlay::hide(); // cannot finish this recording; do not leave the overlay up
    }
}

/// Escape: while recording, throw the audio away; afterwards, make sure nothing still being transcribed gets inserted.
pub fn cancel() {
    let was_recording = RECORDING.swap(false, Ordering::AcqRel);
    if was_recording {
        mute::stop();
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
            _ if !keep => {} // an accidental tap: nothing was said
            Ok(text) => insert(&handle, text),
            Err(problem) => status::report(&handle, &problem),
        }
    });
}

/// Types the text where the cursor is. The text is never lost: if focus moved to another window, or typing fails for any reason, it goes to
/// the clipboard and the user is told, so it never lands in the wrong app and never vanishes.
fn insert(app: &AppHandle, text: String) {
    let prefs = Prefs::load(app);
    let text = clean::clean(&text, &prefs.vocabulary, prefs.remove_fillers);
    if text.is_empty() {
        // Silence, noise, or only filler sounds: say so, or the user cannot tell this from a failure.
        return overlay::notice("Didn't catch any speech", prefs.overlay_top, prefs.theme());
    }
    crate::stats::record(app, text.split_whitespace().count() as u32, LAST_MS.load(Ordering::Acquire) as f32 / 1000.0);
    let routed = rules::run(Trigger::Dictation, &text);
    let (original, text, output, via) = (text, routed.text, routed.output, routed.matched.join(", "));
    if output == Output::Preview {
        return crate::preview::show(app, original, text, via, TARGET.load(Ordering::Acquire));
    }
    deliver(app, text, output == Output::Copy || prefs.output_copy, &prefs);
}

/// Whether focus is still in the window dictation started in (or cannot be told).
fn on_target() -> bool {
    let (target, now) = (TARGET.load(Ordering::Acquire), platform::foreground_window());
    target == 0 || now == 0 || target == now
}

/// Where the text goes: the clipboard when asked (a rule's output or the Output Action setting) or when focus moved to another window since
/// dictation started (so it never lands in the wrong app), otherwise into the focused app.
fn deliver(app: &AppHandle, text: String, copy: bool, prefs: &Prefs) {
    if !on_target() {
        match platform::copy_text(&text) {
            Ok(()) => status::report(app, "You switched windows, so the text was copied to the clipboard. Paste it with Ctrl+V."),
            Err(e) => status::report(app, &format!("You switched windows and the text could not be copied: {e}")),
        }
        return;
    }
    if copy {
        match platform::copy_text(&text) {
            Ok(()) => overlay::notice("Copied to clipboard", prefs.overlay_top, prefs.theme()),
            Err(e) => status::report(app, &format!("Could not copy the text: {e}")),
        }
        return;
    }
    let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
    let (handle, context, submit, backup) = (app.clone(), prefs.context_paste, prefs.auto_submit.clone(), text.clone());
    // Own thread: insertion can wait a few seconds for modifier keys to be released, and must not hold up the speech thread.
    let spawned = std::thread::Builder::new().name("insert".into()).stack_size(512 * 1024).spawn(move || {
        insert_text(&handle, text, &options, context, &submit);
    });
    if spawned.is_err() {
        let _ = platform::copy_text(&backup); // the text is never lost, whatever fails
        status::report(app, "Could not type the text, so it is on your clipboard. Paste it with Ctrl+V.");
    }
}

/// Runs on the insert thread. Context-Aware Paste reads the few characters before the cursor and fits the text to them; Auto Submit then
/// presses its key. Both are off by default, and the submit key is only pressed after the text really went in.
fn insert_text(app: &AppHandle, text: String, options: &InsertOptions, context: bool, submit: &str) {
    let text = match context.then(platform::text_before_cursor).flatten() {
        Some(before) => self::context::fit(&text, &before),
        None => text,
    };
    if let Err(e) = platform::insert(&format!("{text} "), options) {
        return status::report(app, &e.to_string());
    }
    let result = match submit {
        "Enter" => platform::press_submit(false),
        "Ctrl+Enter" => platform::press_submit(true),
        _ => Ok(()),
    };
    if let Err(e) = result {
        status::report(app, &format!("Could not press the submit key: {e}"));
    }
}
