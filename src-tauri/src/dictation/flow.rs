//! File: dictation/flow.rs
//! Purpose: Who owns a dictation and how it ends, as one pure state machine: events in (each callback names its recording id), effects out as data. Whichever of a stop, Escape, an end of the recording itself (limit, deadline, broken stream, tick failure) or a failure ends a recording first takes its field and gets the one effect that stops it; the others find nothing, and a callback of an earlier recording is ignored. The machine also owns the hotkey state (key down, press time, regrip ticket, last stop) and the recording's vocabulary snapshot; the regrip wait is an effect (`Timer`) the owner runs on the audio thread and feeds back as `Regrip`. Delivery stays with its owner; the clipboard rescue is requested by `finish` on every failure that kept words.
//! Contents: Snapshot — the settings one recording reads at its start; Timer — one regrip wait; State — phase, recording id, key state, final-stop latch, field and snapshot; Event — what happened; Effect — what to do about it; State::apply — the transitions; Finish / finish — what to hide and where a finished result goes.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::clean::Cleaner;
use super::mode::{confirm_stop, decide, Action, Mode};
use super::routing::{route, Route};
use crate::audio::{End, Loss};
use crate::platform::Target;
use crate::speech;

/// A press that starts the next recording must not follow a stop by less than this (key bounce).
const BOUNCE: Duration = Duration::from_millis(80);

/// Where a dictation is. Stopping lasts until the audio stop is issued, so the next recording never opens the microphone before the last one is closed.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Recording,
    Stopping,
}

/// The settings one recording reads at its start: whether captions show, and its vocabulary compiled once for every caption and the final text.
#[derive(Clone)]
pub struct Snapshot {
    pub show_overlay: bool,
    pub cleaner: Arc<Cleaner>,
}

impl PartialEq for Snapshot {
    /// Two snapshots are equal when they show captions alike and share one compiled vocabulary.
    fn eq(&self, other: &Snapshot) -> bool {
        self.show_overlay == other.show_overlay && Arc::ptr_eq(&self.cleaner, &other.cleaner)
    }
}

impl std::fmt::Debug for Snapshot {
    /// Prints the caption setting only; the compiled vocabulary may hold the user's words.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot").field("show_overlay", &self.show_overlay).finish_non_exhaustive()
    }
}

/// One regrip wait: the stop request `ticket` for recording `id` (0 when none is published), and whether it came from letting go of the key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timer {
    pub ticket: u64,
    pub id: u64,
    pub from_release: bool,
}

/// What happened to a dictation. Every callback carries the id of the recording it belongs to.
pub enum Event {
    /// The hotkey went down or up at `at`; `mode` is the user's dictation mode.
    Key { down: bool, at: Instant, mode: Mode },
    /// A press that starts a recording in the field `Target`.
    Start(Target),
    /// The speech engine named the recording, which reads `Snapshot`.
    Published(u64, Snapshot),
    /// The engine failed or was unloaded while recording `id` was open.
    Failed(u64),
    /// Recording `id` ended by itself for the reason `End`.
    Ended(u64, End),
    /// The regrip window of a stop request is over.
    Regrip(Timer),
    /// Throw away recording `id`, or whichever runs when None (Escape).
    Abandon(Option<u64>),
    /// The audio stop of the ended recording was issued (or its cleanup is over) at the instant: the next press may start.
    StopIssued(Instant),
    /// The engine delivered the final text of recording `id`: its snapshot is no longer needed.
    Finished(u64),
}

/// What the owner of the dictation must do.
#[derive(Debug, PartialEq)]
pub enum Effect {
    /// Run the preflight for a press that starts a recording, then send `Event::Start`.
    StartRequested,
    /// Ask the speech engine for a recording id.
    BeginSpeech,
    /// Open the microphone, mute and overlay for recording `id`.
    OpenMic(u64),
    /// Send `Event::Regrip` for the wait after the regrip window.
    Timer(Timer),
    /// Tell the user the recording limit was reached: the recording is full (`End::Cap`). A deadline without a full recording means the microphone sent too little sound, not that the user reached the limit, so it gives no notice.
    LimitReached,
    /// Recording `id` was ended: stop the microphone and let the engine finish its words, dictated into `target`, cleaned with `snapshot`.
    StopMic { id: u64, target: Target, snapshot: Option<Snapshot> },
    /// Throw the recording away: microphone, mute, text and overlay.
    Discard,
}

/// The dictation's ownership state. The recording id is kept after a stop, so the last captions still show while the engine finishes; an abandon clears it.
pub struct State {
    phase: Phase,
    id: Option<u64>,
    /// The id whose end is final: its engine failed or the recording ended by itself. Its stop needs no regrip check, so a regrip cannot cancel it; a failure that arrives before the id is published is acted on when it is.
    final_stop: Option<u64>,
    target: Option<Target>,
    key_down: bool,
    pressed_at: Option<Instant>,
    last_stop: Option<Instant>,
    /// Count of stop requests: only the newest may end the recording.
    stops: u64,
    /// The release stop request still waiting out the regrip window. A press meanwhile is the key coming back, not a new press.
    regrip: Option<u64>,
    /// The recording the snapshot belongs to, and what it holds.
    snapshot: Option<(u64, Snapshot)>,
}

impl State {
    /// An idle state with no recording.
    pub const fn new() -> State {
        State {
            phase: Phase::Idle,
            id: None,
            final_stop: None,
            target: None,
            key_down: false,
            pressed_at: None,
            last_stop: None,
            stops: 0,
            regrip: None,
            snapshot: None,
        }
    }

    /// Whether a recording is running now (not while one is being ended).
    pub fn is_recording(&self) -> bool {
        self.phase == Phase::Recording
    }

    /// Whether `id` is the recording that was named last, running or just ended, and not abandoned: its captions show and its text-model warm-up goes on. Input: the recording id.
    pub fn is_current(&self, id: u64) -> bool {
        self.id == Some(id)
    }

    /// The snapshot of recording `id`, None when another recording owns it or it is over. Input: the id of a caption.
    pub fn caption_snapshot(&self, id: u64) -> Option<Snapshot> {
        self.snapshot.as_ref().filter(|(owner, _)| *owner == id).map(|(_, snapshot)| snapshot.clone())
    }

    /// Whether recording `id` is the one running now. Input: the id, or None for whichever runs.
    pub fn running(&self, id: Option<u64>) -> bool {
        self.is_recording() && id.is_none_or(|id| self.id == Some(id))
    }

    /// Takes ownership of the running recording: the phase moves to stopping and the field is handed over in one step. Returns the field.
    fn take(&mut self) -> Target {
        self.phase = Phase::Stopping;
        self.regrip = None;
        self.target.take().unwrap_or_default()
    }

    /// Numbers a new stop request, which supersedes every older one. Inputs: the recording it is for (0 when none is published) and whether the key release made it. Returns the timer effect.
    fn request_stop(&mut self, id: u64, from_release: bool) -> Effect {
        self.stops += 1;
        if from_release {
            self.regrip = Some(self.stops);
        }
        Effect::Timer(Timer { ticket: self.stops, id, from_release })
    }

    /// What a key press or release means now. Inputs: whether the key is down, when, and the dictation mode. Returns the effects.
    fn key(&mut self, down: bool, at: Instant, mode: Mode) -> Vec<Effect> {
        self.key_down = down;
        if down && self.regrip.is_some() {
            return Vec::new(); // the key came straight back: recording simply continues
        }
        if down {
            self.pressed_at = Some(at);
        }
        let held = self.pressed_at.map_or(Duration::ZERO, |t| at.saturating_duration_since(t));
        match decide(mode, down, self.is_recording(), held) {
            Action::Start if !self.last_stop.is_some_and(|t| at.saturating_duration_since(t) < BOUNCE) => vec![Effect::StartRequested],
            Action::Stop => vec![self.request_stop(self.id.unwrap_or(0), !down)],
            _ => Vec::new(),
        }
    }

    /// Ends the regrip wait of `timer`: the stop stands when the key stayed up and no newer stop exists, or when the end is final. Returns the effect that stops the microphone, if it does.
    fn regrip(&mut self, timer: Timer) -> Vec<Effect> {
        if self.regrip == Some(timer.ticket) {
            self.regrip = None;
        }
        let confirmed = confirm_stop(timer.from_release, self.key_down, self.stops != timer.ticket);
        if !self.running(Some(timer.id)) || !(confirmed || self.final_stop == Some(timer.id)) {
            return Vec::new();
        }
        let snapshot = self.snapshot.as_ref().map(|(_, snapshot)| snapshot.clone());
        vec![Effect::StopMic { id: timer.id, target: self.take(), snapshot }]
    }

    /// Applies one event. Input: the event. Returns the effects to run, in order; empty when the event is stale or lost the race.
    pub fn apply(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::Key { down, at, mode } => self.key(down, at, mode),
            Event::Start(target) if self.phase == Phase::Idle => {
                self.phase = Phase::Recording;
                (self.id, self.final_stop, self.target) = (None, None, Some(target));
                vec![Effect::BeginSpeech]
            }
            Event::Published(id, snapshot) if self.is_recording() && self.id.is_none() => {
                self.id = Some(id);
                self.snapshot = Some((id, snapshot));
                let mut effects = vec![Effect::OpenMic(id)];
                if self.final_stop == Some(id) {
                    effects.push(self.request_stop(id, false));
                }
                effects
            }
            Event::Failed(id) if self.is_recording() && self.id.is_none_or(|current| current == id) => {
                self.final_stop = Some(id);
                if self.id.is_some() {
                    vec![self.request_stop(id, false)]
                } else {
                    Vec::new()
                }
            }
            Event::Ended(id, end) if self.running(Some(id)) => {
                self.final_stop = Some(id);
                let limit = (end == End::Cap).then_some(Effect::LimitReached);
                limit.into_iter().chain([self.request_stop(id, false)]).collect()
            }
            Event::Regrip(timer) => self.regrip(timer),
            Event::Abandon(id) if self.running(id) => {
                self.take();
                (self.id, self.snapshot) = (None, None);
                vec![Effect::Discard]
            }
            Event::StopIssued(at) if self.phase == Phase::Stopping => {
                self.phase = Phase::Idle;
                self.last_stop = Some(at);
                Vec::new()
            }
            Event::Finished(id) => {
                if self.snapshot.as_ref().is_some_and(|(owner, _)| *owner == id) {
                    self.snapshot = None;
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}

/// What a finished recording asks for: which overlay to hide and where its result goes.
pub struct Finish {
    pub hide: u64,
    pub route: Route,
}

/// Decides what the engine's answer for recording `id` asks for. Inputs: the id, whether the recording was long enough to keep, what it lost and the engine's result. Returns the overlay to hide and the route; words kept by a failed engine always route to the clipboard rescue.
pub fn finish(id: u64, keep: bool, loss: Option<Loss>, result: Result<String, speech::Failure>) -> Finish {
    Finish { hide: id, route: route(result, keep, loss) }
}
