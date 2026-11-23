//! File: dictation/mod.rs
//! Purpose: Dictation flow glue: hotkey -> mic -> overlay -> speech engine -> insertion. Ownership and completion are decided by the pure `flow` state machine; this file runs the effects it returns.
//! Contents: init — start the audio and overlay threads; on_hotkey — hotkey to start / stop via `mode`, never blocks;
//! cancel — Escape discards a recording in progress (never text already spoken). A release waits out a short regrip window before it stops, so a shift of finger pressure does not end the recording.
//! The text of a finished dictation is typed at the cursor or, when that cannot be done, left on the clipboard: it is never dropped.

use crate::lock::lock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod clean;
mod context;
mod deliver;
mod flow;
mod mode;
pub mod mute;
pub mod recovery;
mod routing;
mod timing;

use tauri::AppHandle;

use clean::Cleaner;
use flow::{finish, Effect, Event, State};
use mode::{confirm_stop, decide, Action, Mode};
use routing::{dispatch, Live};
use timing::Timeline;

use crate::platform::{self, overlay, Target};
use crate::prefs::Prefs;
use crate::rules::model::Trigger;
use crate::{audio, rules, settings, speech, status};

/// Recording runs this long after the key is released: the last syllable is not clipped, and a key that comes straight back down (a shift
/// of finger pressure on a laptop key) continues the same recording instead of ending it.
const REGRIP: Duration = Duration::from_millis(200);
/// A press that starts the next recording must not follow a stop by less than this (key bounce).
const BOUNCE: Duration = Duration::from_millis(80);

/// The one dictation's ownership state; every change goes through `send`.
static FLOW: Mutex<State> = Mutex::new(State::new());
static LAST_STOP: Mutex<Option<Instant>> = Mutex::new(None);
/// When the hotkey last went down; Hold or Toggle needs the press length on release.
static PRESSED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Whether the hotkey is down now, and a count of stop requests: only the newest one may end the recording.
static KEY_DOWN: AtomicBool = AtomicBool::new(false);
static STOPS: AtomicU64 = AtomicU64::new(0);
/// The stop request (from a release) still waiting out the regrip window, or 0. A press meanwhile is the key coming back, not a new press.
static REGRIP_OF: AtomicU64 = AtomicU64::new(0);
/// What the live caption and the final text use, read once when the dictation starts instead of from disk for every sentence.
static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);

/// The settings one recording reads at its start: whether captions show, and its vocabulary compiled once for every caption and the final text.
#[derive(Clone)]
struct Snapshot {
    show_overlay: bool,
    cleaner: Arc<Cleaner>,
}

/// What is known about one finished recording: its length for the statistics, the window it was dictated into, its vocabulary and its stage timings. Taken when the user
/// stops, so a dictation started right after cannot change them.
#[derive(Clone)]
struct Spoken {
    ms: u64,
    target: Target,
    cleaner: Arc<Cleaner>,
    timeline: Timeline,
}

impl Spoken {
    /// Marks that `stage` was reached and writes the timing line with the stages so far (no text). Used where a dictation ends, so every route shows how far it got without claiming an insertion. Input: the stage name.
    fn log(&mut self, stage: &'static str) {
        self.timeline.mark(stage);
        crate::log::note(&self.timeline.line(self.ms, crate::speech::process::threads()));
    }
}

/// Applies an event to the dictation's state. Input: the event. Returns the effects to run.
fn send(event: Event) -> Vec<Effect> {
    lock(&FLOW).apply(event)
}

/// Held by whoever ended a recording until it has issued the audio stop (and the rest of the ending): dropping it, however the ending went, lets the next press start.
struct Issued;

impl Drop for Issued {
    /// Reports that the stop was issued.
    fn drop(&mut self) {
        send(Event::StopIssued);
    }
}

/// Whether a recording is in progress. Returns true from the hotkey press until its stop is taken.
pub fn is_recording() -> bool {
    lock(&FLOW).is_recording()
}

/// Starts the audio and overlay threads and registers the speech callbacks. Input: the app. Returns why a thread could not start.
pub fn init(app: &AppHandle) -> std::io::Result<()> {
    let handle = app.clone();
    audio::init(Arc::new(move |problem| status::report(&handle, &problem)))?;
    speech::on_partial(show_caption);
    let handle = app.clone();
    speech::on_failed(move |id| run(&handle, send(Event::Failed(id)), &Timeline::start()));
    mute::init(app);
    overlay::init()
}

/// A sentence was recognised: show it in the pill, cleaned the same way as the text that will be typed.
fn show_caption(id: u64, index: usize, text: &str) {
    if !lock(&FLOW).is_current(id) {
        return; // belongs to an earlier dictation
    }
    let Some(snapshot) = lock(&SNAPSHOT).clone() else { return };
    let shown = snapshot.cleaner.clean(text);
    if snapshot.show_overlay && !shown.is_empty() {
        overlay::caption(id, index, &shown);
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
        let mut pressed = lock(&PRESSED_AT);
        if down {
            *pressed = Some(Instant::now());
        }
        pressed.map_or(Duration::ZERO, |t| t.elapsed())
    };
    match decide(mode, down, is_recording(), held) {
        Action::Start if !bounced() => start(app),
        Action::Stop => stop(app, !down, current_id()),
        _ => {}
    }
}

/// The id of the recording named last, 0 when it is not published yet. A stop that names 0 finds no recording.
fn current_id() -> u64 {
    lock(&FLOW).id().unwrap_or(0)
}

/// The dictation mode from settings.
fn current_mode() -> Mode {
    Mode::parse(settings::get_str("dictation_mode").as_deref())
}

/// Whether the last stop was so recent that a press now is key bounce.
fn bounced() -> bool {
    lock(&LAST_STOP).is_some_and(|t| t.elapsed() < BOUNCE)
}

/// Starts a dictation: the speech model begins loading, the microphone opens and the overlay shows. Input: the app.
fn start(app: &AppHandle) {
    if crate::snippets::ask::is_open() {
        return; // the question window has the keyboard
    }
    crate::dictionary::cancel(); // a lookup or card gives way to a dictation
    status::clear(app);
    let prefs = Prefs::load();
    if !speech::has_model(app) {
        // Nothing could be transcribed: say so now instead of recording, loading and then failing.
        status::report_as(app, "No speech model. Download one in Settings, Models.", "Download a model to start dictating");
        return;
    }
    if !send(Event::Start(platform::capture_target())).contains(&Effect::BeginSpeech) {
        return; // the last recording is still being ended: this press is dropped, not started under its late stop
    }
    let id = speech::begin(app); // the model loads while the user speaks
    if let Some(task) = rules::model_task_for(Trigger::Dictation) {
        crate::text::prepare(task); // and so does the text model, if a rule will use it
    }
    let snapshot = Snapshot { show_overlay: prefs.show_overlay, cleaner: Arc::new(Cleaner::new(&prefs.vocabulary, prefs.remove_fillers)) };
    *lock(&SNAPSHOT) = Some(snapshot);
    let effects = send(Event::Published(id));
    if effects.contains(&Effect::OpenMic(id)) {
        open_mic(app, id, &prefs);
    }
    run(app, effects, &Timeline::start());
}

/// Opens the microphone, the mute and the overlay for recording `id`. Inputs: the app, the id and the preferences read at the start.
fn open_mic(app: &AppHandle, id: u64, prefs: &Prefs) {
    mute::start();
    if prefs.show_overlay {
        overlay::show(id, prefs.overlay_top, prefs.theme());
    }
    let sink = Arc::new(move |chunk| speech::feed(id, chunk)); // speech is recognised during the recording, not after it
    let handle = app.clone();
    audio::start(audio::Config {
        mic: prefs.mic.clone(),
        max_secs: prefs.max_record_secs,
        linger: Duration::from_secs(prefs.mic_linger_secs),
        sink,
        on_cap: Arc::new(move || run(&handle, send(Event::Capped(id)), &Timeline::start())),
        on_fail: Arc::new(move || abandon(Some(id))), // no microphone: do not leave the overlay up and the speakers muted
    });
}

/// Runs the effects of an event, in order. Inputs: the app, the effects and the clock of the stop they belong to. `BeginSpeech` and `OpenMic` are run by `start`.
fn run(app: &AppHandle, effects: Vec<Effect>, timeline: &Timeline) {
    for effect in effects {
        match effect {
            Effect::BeginSpeech | Effect::OpenMic(_) => {}
            Effect::StopSoon(id) => stop(app, false, id),
            Effect::Capped => {
                status::report_as(app, "Reached the recording limit. Everything said so far is being typed.", "Recording limit reached")
            }
            Effect::StopMic { id, target } => end_recording(app, id, target, timeline.clone()),
            Effect::Discard => discard(),
        }
    }
}

/// Ends recording `id` after the regrip window, and only that recording: the id comes from the caller, never read again here, so a callback of an earlier recording cannot end the next one.
/// `from_release`: the stop came from letting go of the key, so it waits to see whether the key comes straight back.
fn stop(app: &AppHandle, from_release: bool, id: u64) {
    let ticket = STOPS.fetch_add(1, Ordering::AcqRel) + 1;
    if from_release {
        REGRIP_OF.store(ticket, Ordering::Release);
    }
    let timeline = Timeline::start();
    let handle = app.clone();
    // The overlay stays up, grey and still, while the last words are recognised: it shows Cue heard you and is working.
    let spawned = crate::guard::spawn("dictation-stop", 128 * 1024, move || {
        std::thread::sleep(REGRIP);
        let _ = REGRIP_OF.compare_exchange(ticket, 0, Ordering::AcqRel, Ordering::Acquire);
        let superseded = STOPS.load(Ordering::Acquire) != ticket;
        let confirmed = confirm_stop(from_release, KEY_DOWN.load(Ordering::Acquire), superseded);
        run(&handle, send(Event::Stop { id, confirmed }), &timeline);
    });
    if spawned.is_err() {
        REGRIP_OF.store(0, Ordering::Release);
        abandon(Some(id)); // cannot finish this recording: the hold lasts through the cleanup, and a recording that is not ours is left alone
    }
}

/// Ends the recording it owns: unmutes, stops the microphone and hands the last audio to the engine. Inputs: the app, the recording id, its window and the clock of this stop.
fn end_recording(app: &AppHandle, id: u64, target: Target, mut timeline: Timeline) {
    let _issued = Issued;
    let snapshot = lock(&SNAPSHOT).clone();
    mute::stop();
    timeline.mark("regrip");
    *lock(&LAST_STOP) = Some(Instant::now());
    let min_samples = (audio::RATE as u64 * Prefs::min_record_ms() / 1000) as usize; // shorter is an accidental tap
    let app = app.clone();
    audio::stop(move |tail, total| {
        let cleaner = snapshot.map(|s| s.cleaner).unwrap_or_default();
        let mut spoken = Spoken { ms: total as u64 * 1000 / audio::RATE as u64, target, cleaner, timeline };
        let keep = total >= min_samples.max(1);
        if !keep {
            overlay::hide(id); // nothing to wait for
        }
        speech::feed(id, tail);
        spoken.timeline.mark("tail");
        transcribe_and_insert(&app, id, keep, spoken);
    });
}

/// Escape: while recording, throw the audio away. Once the user has stopped, what was said is never dropped: it is still recognised and delivered.
pub fn cancel() {
    abandon(None);
}

/// Throws away the running recording (its microphone, mute, text and overlay) when it is the one named, or whichever runs when None; does nothing when another ending came first. Input: the recording id.
fn abandon(id: Option<u64>) {
    if send(Event::Abandon(id)).contains(&Effect::Discard) {
        discard();
    }
}

/// The cleanup of an abandoned recording, held by `Issued` until it is over.
fn discard() {
    let _issued = Issued;
    mute::stop();
    *lock(&LAST_STOP) = Some(Instant::now());
    audio::stop(|_, _| {});
    speech::cancel();
    overlay::dismiss();
}

/// `keep` is false for an accidental tap: its text is thrown away.
fn transcribe_and_insert(app: &AppHandle, id: u64, keep: bool, mut spoken: Spoken) {
    let handle = app.clone();
    speech::finish(id, move |result| {
        spoken.timeline.mark("final");
        let done = finish(id, keep, result);
        overlay::hide(done.hide); // ignored when the user already started the next dictation, which owns the overlay now
        dispatch(done.route, Live { app: &handle, spoken });
    });
}

#[cfg(test)]
mod tests;
