//! File: dictation/mod.rs
//! Purpose: Dictation flow glue: hotkey -> mic -> overlay -> speech engine -> insertion. Ownership and completion are decided by the pure `flow` state machine; this file runs the effects it returns.
//! Contents: init — start the audio and overlay threads; on_hotkey — hands the key to the `flow` machine and runs its effects on the shortcut effects thread (never the keyboard hook thread);
//! cancel — Escape discards a recording in progress (never text already spoken). A release waits out a short regrip window (a timer effect run on the audio thread) before it stops, so a shift of finger pressure does not end the recording.
//! The text of a finished dictation is typed at the cursor or, when that cannot be done, left on the clipboard: it is never dropped.

use crate::lock::lock;
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
use flow::{finish, Effect, Event, Snapshot, State};
use mode::Mode;
use routing::{dispatch, Live};
use timing::Timeline;

use crate::platform::{self, overlay, Target};
use crate::prefs::Prefs;
use crate::rules::model::Trigger;
use crate::{audio, rules, settings, speech, status};

/// Recording runs this long after the key is released: the last syllable is not clipped, and a key that comes straight back down (a shift
/// of finger pressure on a laptop key) continues the same recording instead of ending it.
const REGRIP: Duration = Duration::from_millis(200);
/// The one dictation's ownership state; every change goes through `send`.
static FLOW: Mutex<State> = Mutex::new(State::new());

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
        send(Event::StopIssued(Instant::now()));
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
    let Some(snapshot) = lock(&FLOW).caption_snapshot(id) else { return };
    let shown = snapshot.cleaner.clean(text);
    if snapshot.show_overlay && !shown.is_empty() {
        overlay::caption(id, index, &shown);
    }
}

/// `down` is the hotkey state; the `flow` machine decides what it means in the user's dictation mode.
pub fn on_hotkey(app: &AppHandle, down: bool) {
    run(app, send(Event::Key { down, at: Instant::now(), mode: current_mode() }), &Timeline::start());
}

/// The dictation mode from settings.
fn current_mode() -> Mode {
    Mode::parse(settings::get_str("dictation_mode").as_deref())
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
    let snapshot = Snapshot { show_overlay: prefs.show_overlay, cleaner: Arc::new(Cleaner::new(&prefs.vocabulary, prefs.remove_fillers)) };
    let effects = send(Event::Published(id, snapshot));
    if effects.contains(&Effect::OpenMic(id)) {
        if let Some(task) = rules::model_task_for(Trigger::Dictation) {
            // and so does the text model, if a rule will use it, until this dictation is abandoned or a newer one starts
            crate::text::prepare(task, crate::text::Cancel::new(move || !lock(&FLOW).is_current(id)));
        }
        open_mic(app, id, &prefs);
    }
    run(app, effects, &Timeline::start());
}

/// Opens the microphone, the mute and the overlay for recording `id`. Inputs: the app, the id and the preferences read at the start.
fn open_mic(app: &AppHandle, id: u64, prefs: &Prefs) {
    mute::start(|| lock(&FLOW).running(Some(id)));
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
        on_end: Arc::new(move |end| run(&handle, send(Event::Ended(id, end)), &Timeline::start())),
        on_fail: Arc::new(move || abandon(Some(id))), // no microphone: do not leave the overlay up and the speakers muted
    });
}

/// Runs the effects of an event, in order. Inputs: the app, the effects and the clock of the stop they belong to. `BeginSpeech` and `OpenMic` are run by `start`.
fn run(app: &AppHandle, effects: Vec<Effect>, timeline: &Timeline) {
    for effect in effects {
        match effect {
            Effect::BeginSpeech | Effect::OpenMic(_) => {}
            Effect::StartRequested => start(app),
            Effect::Timer(timer) => {
                // The overlay stays up, grey and still, while the last words are recognised: it shows Cue heard you and is working.
                let (handle, timeline) = (app.clone(), timeline.clone());
                audio::after(REGRIP, move || run(&handle, send(Event::Regrip(timer)), &timeline));
            }
            Effect::LimitReached => {
                status::report_as(app, "Reached the recording limit. Everything said so far is being typed.", "Recording limit reached")
            }
            Effect::StopMic { id, target, snapshot } => end_recording(app, id, target, snapshot, timeline.clone()),
            Effect::Discard => discard(),
        }
    }
}

/// Ends the recording it owns: unmutes, stops the microphone and hands the last audio to the engine. Inputs: the app, the recording id, its window, its snapshot and the clock of this stop.
fn end_recording(app: &AppHandle, id: u64, target: Target, snapshot: Option<Snapshot>, mut timeline: Timeline) {
    let _issued = Issued;
    mute::stop();
    timeline.mark("regrip");
    let min_samples = (audio::RATE as u64 * Prefs::min_record_ms() / 1000) as usize; // shorter is an accidental tap
    let app = app.clone();
    audio::stop(move |heard| {
        let (tail, total, loss) = (heard.tail, heard.total, heard.loss);
        let cleaner = snapshot.map(|s| s.cleaner).unwrap_or_default();
        let mut spoken = Spoken { ms: total as u64 * 1000 / audio::RATE as u64, target, cleaner, timeline };
        let keep = total >= min_samples.max(1);
        if !keep {
            overlay::hide(id); // nothing to wait for
        }
        speech::feed(id, tail);
        spoken.timeline.mark("tail");
        transcribe_and_insert(&app, id, keep, loss, spoken);
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
    audio::stop(|_| {});
    speech::cancel();
    overlay::dismiss();
}

/// `keep` is false for an accidental tap: its text is thrown away. `loss` is what the recording lost; words from a lossy one are rescued, not inserted.
fn transcribe_and_insert(app: &AppHandle, id: u64, keep: bool, loss: Option<audio::Loss>, mut spoken: Spoken) {
    let handle = app.clone();
    speech::finish(id, move |result| {
        spoken.timeline.mark("final");
        let done = finish(id, keep, loss, result);
        send(Event::Finished(id));
        overlay::hide(done.hide); // ignored when the user already started the next dictation, which owns the overlay now
        dispatch(done.route, Live { app: &handle, spoken });
    });
}

#[cfg(test)]
mod tests;
