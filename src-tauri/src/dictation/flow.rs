//! File: dictation/flow.rs
//! Purpose: Who owns a dictation and how it ends, as one pure state machine: events in (each callback names its recording id), effects out as data. Whichever of a stop, Escape, an end of the recording itself (limit, deadline, broken stream, tick failure) or a failure ends a recording first takes its field and gets the one effect that stops it; the others find nothing, and a callback of an earlier recording is ignored. Starting, regrip timing and delivery stay with their owners; the clipboard rescue is requested by `finish` on every failure that kept words.
//! Contents: State — phase, recording id, final-stop latch and field; Event — what happened; Effect — what to do about it; State::apply — the transitions; Finish / finish — what to hide and where a finished result goes.

use super::routing::{route, Route};
use crate::audio::{End, Loss};
use crate::platform::Target;
use crate::speech;

/// Where a dictation is. Stopping lasts until the audio stop is issued, so the next recording never opens the microphone before the last one is closed.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Recording,
    Stopping,
}

/// What happened to a dictation. Every callback carries the id of the recording it belongs to.
pub enum Event {
    /// A press that starts a recording in the field `Target`.
    Start(Target),
    /// The speech engine named the recording.
    Published(u64),
    /// The engine failed or was unloaded while recording `id` was open.
    Failed(u64),
    /// Recording `id` ended by itself for the reason `End`.
    Ended(u64, End),
    /// The regrip window of recording `id` is over; `confirmed` says the stop still stands (the key stayed up, no newer stop).
    Stop { id: u64, confirmed: bool },
    /// Throw away recording `id`, or whichever runs when None (Escape).
    Abandon(Option<u64>),
    /// The audio stop of the ended recording was issued (or its cleanup is over): the next press may start.
    StopIssued,
}

/// What the owner of the dictation must do.
#[derive(Debug, PartialEq)]
pub enum Effect {
    /// Ask the speech engine for a recording id.
    BeginSpeech,
    /// Open the microphone, mute and overlay for recording `id`.
    OpenMic(u64),
    /// End recording `id` after the regrip window.
    StopSoon(u64),
    /// Tell the user the recording limit was reached.
    LimitReached,
    /// Recording `id` was ended: stop the microphone and let the engine finish its words, dictated into `target`.
    StopMic { id: u64, target: Target },
    /// Throw the recording away: microphone, mute, text and overlay.
    Discard,
}

/// The dictation's ownership state. The recording id is kept after the end, so the last captions still show while the engine finishes.
pub struct State {
    phase: Phase,
    id: Option<u64>,
    /// The id whose end is final: its engine failed or the recording ended by itself. Its stop needs no regrip check, so a regrip cannot cancel it; a failure that arrives before the id is published is acted on when it is.
    final_stop: Option<u64>,
    target: Option<Target>,
}

impl State {
    /// An idle state with no recording.
    pub const fn new() -> State {
        State { phase: Phase::Idle, id: None, final_stop: None, target: None }
    }

    /// Whether a recording is running now (not while one is being ended).
    pub fn is_recording(&self) -> bool {
        self.phase == Phase::Recording
    }

    /// The id of the recording that was named last, None before it is published.
    pub fn id(&self) -> Option<u64> {
        self.id
    }

    /// Whether `id` is the recording that was named last, running or just ended. Input: the id of a caption.
    pub fn is_current(&self, id: u64) -> bool {
        self.id == Some(id)
    }

    /// Whether recording `id` is the one running now. Input: the id, or None for whichever runs.
    fn running(&self, id: Option<u64>) -> bool {
        self.is_recording() && id.is_none_or(|id| self.id == Some(id))
    }

    /// Takes ownership of the running recording: the phase moves to stopping and the field is handed over in one step. Returns the field.
    fn take(&mut self) -> Target {
        self.phase = Phase::Stopping;
        self.target.take().unwrap_or_default()
    }

    /// Applies one event. Input: the event. Returns the effects to run, in order; empty when the event is stale or lost the race.
    pub fn apply(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::Start(target) if self.phase == Phase::Idle => {
                *self = State { phase: Phase::Recording, id: None, final_stop: None, target: Some(target) };
                vec![Effect::BeginSpeech]
            }
            Event::Published(id) if self.is_recording() && self.id.is_none() => {
                self.id = Some(id);
                let mut effects = vec![Effect::OpenMic(id)];
                if self.final_stop == Some(id) {
                    effects.push(Effect::StopSoon(id));
                }
                effects
            }
            Event::Failed(id) if self.is_recording() && self.id.is_none_or(|current| current == id) => {
                self.final_stop = Some(id);
                if self.id.is_some() {
                    vec![Effect::StopSoon(id)]
                } else {
                    Vec::new()
                }
            }
            Event::Ended(id, end) if self.running(Some(id)) => {
                self.final_stop = Some(id);
                let limit = matches!(end, End::Cap | End::Deadline).then_some(Effect::LimitReached);
                limit.into_iter().chain([Effect::StopSoon(id)]).collect()
            }
            Event::Stop { id, confirmed } if self.running(Some(id)) && (confirmed || self.final_stop == Some(id)) => {
                vec![Effect::StopMic { id, target: self.take() }]
            }
            Event::Abandon(id) if self.running(id) => {
                self.take();
                vec![Effect::Discard]
            }
            Event::StopIssued if self.phase == Phase::Stopping => {
                self.phase = Phase::Idle;
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
