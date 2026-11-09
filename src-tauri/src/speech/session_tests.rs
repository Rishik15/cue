//! File: speech/session_tests.rs
//! Purpose: Protect every rule of a dictation session with a fake clock and a scripted worker: ordering of overlapping dictations, crash
//! replay giving the text an uninterrupted run gives, the one retry, every deadline, cancel by id, the idle unload, and a property test over
//! random event sequences (each callback fires once, nothing of a cancelled recording appears, End A always precedes Begin B).
//! Contents: Fake — the scripted worker; Sim — the session plus the fake and a clock; table tests; the proptest.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use proptest::prelude::*;

use super::events::{Effect, Event, Pick, Unload};
use super::protocol::{Adapter, EngineSpec, Reply, Request, Segmented, VERSION};
use super::session::Session;
use super::take::Done;

const CLAUSE: usize = 4;
const CAP: Duration = Duration::from_secs(1200);
static SPEC_A: EngineSpec = EngineSpec::int8(Adapter::Segmented(Segmented::Transducer), true, Some("nemo_transducer"), None);
static SPEC_B: EngineSpec = EngineSpec::int8(Adapter::Segmented(Segmented::Whisper), false, None, Some(""));
const PICK_A: Pick = Pick { id: "a", spec: &SPEC_A };
const PICK_B: Pick = Pick { id: "b", spec: &SPEC_B };

type Results = Arc<Mutex<Vec<(u64, Result<String, String>)>>>;

/// A scripted worker: every `CLAUSE` samples form a clause whose text is the samples themselves, so the text depends only on the audio it was
/// sent from the last release point. Like the streaming engine it commits a clause as soon as it is found but releases audio only at an
/// endpoint (after every second clause, and at the End), always on a clause boundary.
#[derive(Default)]
struct Fake {
    id: u64,
    first: usize,
    buf: Vec<f32>,
    fed: usize,
    clauses: Vec<String>,
    released: usize,
    released_clauses: usize,
}

impl Fake {
    /// Answers one request like the real worker: Partials, the Text of an End, then the Release.
    fn handle(&mut self, request: Request, samples: &[f32]) -> Vec<Reply> {
        let mut out = Vec::new();
        match request {
            Request::Begin { id, clause } => *self = Fake { id, first: clause, released_clauses: clause, ..Fake::default() },
            Request::Audio => {
                self.fed += samples.len();
                self.buf.extend_from_slice(samples);
                while self.buf.len() >= CLAUSE {
                    let chunk: Vec<f32> = self.buf.drain(..CLAUSE).collect();
                    out.push(self.clause(&chunk));
                    if self.clauses.len().is_multiple_of(2) {
                        self.endpoint();
                    }
                }
            }
            Request::End { id } => {
                if !self.buf.is_empty() {
                    let rest = std::mem::take(&mut self.buf);
                    out.push(self.clause(&rest));
                }
                self.endpoint();
                out.push(Reply::Text { id, text: self.clauses.join(" ") });
            }
            Request::Load { .. } => {}
        }
        out.push(Reply::Release { id: self.id, up_to: self.released, clauses: self.released_clauses });
        out
    }

    /// An endpoint: the audio before the unprocessed rest is used up, and so are the clauses it produced.
    fn endpoint(&mut self) {
        self.released = self.fed - self.buf.len();
        self.released_clauses = self.first + self.clauses.len();
    }

    /// Finds one clause in `chunk` and returns its Partial reply.
    fn clause(&mut self, chunk: &[f32]) -> Reply {
        let text = chunk.iter().map(|s| (*s as i64).to_string()).collect::<Vec<_>>().join("-");
        self.clauses.push(text.clone());
        Reply::Partial { id: self.id, index: self.first + self.clauses.len() - 1, text }
    }
}

/// What a request looked like on the wire: `B1` Begin of 1, `A6` audio of 6 samples, `E1` End of 1.
fn tag(request: &Request, samples: &[f32]) -> String {
    match request {
        Request::Begin { id, clause } => format!("B{id}.{clause}"),
        Request::Audio => format!("A{}", samples.len()),
        Request::End { id } => format!("E{id}"),
        Request::Load { .. } => "L".into(),
    }
}

struct Sim {
    session: Session,
    now: Instant,
    worker: Option<(u64, Fake)>,
    inbox: VecDeque<(u64, Option<Reply>)>,
    results: Results,
    reports: Vec<String>,
    logs: Vec<String>,
    partials: Vec<(u64, usize)>,
    wire: Vec<String>,
    log: Vec<String>,
    spawns: Vec<Pick>,
    kills: usize,
    hold_load: bool,
    mute: bool,
}

impl Sim {
    fn new() -> Self {
        let now = Instant::now();
        Sim {
            session: Session::new(now),
            now,
            worker: None,
            inbox: VecDeque::new(),
            results: Arc::default(),
            reports: Vec::new(),
            logs: Vec::new(),
            partials: Vec::new(),
            wire: Vec::new(),
            log: Vec::new(),
            spawns: Vec::new(),
            kills: 0,
            hold_load: false,
            mute: false,
        }
    }

    fn done(&self, id: u64) -> Done {
        let results = self.results.clone();
        Box::new(move |r| results.lock().unwrap().push((id, r)))
    }

    fn event(&mut self, event: Event) {
        let effects = self.session.step(event, self.now);
        for effect in effects {
            self.run(effect);
        }
    }

    fn run(&mut self, effect: Effect) {
        match effect {
            Effect::Spawn { serial, pick } => {
                self.spawns.push(pick);
                self.worker = Some((serial, Fake::default()));
                self.log.push("S".into());
                if !self.hold_load {
                    self.inbox.push_back((serial, Some(Reply::Loaded { ms: 1, version: VERSION })));
                }
            }
            Effect::Send { serial, request, samples } => {
                self.wire.push(tag(&request, &samples));
                self.log.push(tag(&request, &samples));
                if let Some((_, fake)) = self.worker.as_mut().filter(|(s, _)| *s == serial && !self.mute) {
                    self.inbox.extend(fake.handle(request, &samples).into_iter().map(|r| (serial, Some(r))));
                }
            }
            Effect::Kill { serial } => {
                self.kills += 1;
                if self.worker.as_ref().is_some_and(|(s, _)| *s == serial) {
                    self.worker = None;
                }
                self.inbox.retain(|(s, _)| *s != serial);
            }
            Effect::Deliver(done, result) => done(result),
            Effect::Report(message) => self.reports.push(message),
            Effect::Partial { id, index, .. } => self.partials.push((id, index)),
            Effect::Trim => {}
            Effect::Log(line) => self.logs.push(line),
        }
    }

    fn begin(&mut self, id: u64) {
        self.begin_with(id, Ok(PICK_A));
    }

    fn begin_with(&mut self, id: u64, model: Result<Pick, String>) {
        self.event(Event::Begin { id, model, cap: CAP });
    }

    fn audio(&mut self, id: u64, from: i64, to: i64) {
        let samples = (from..to).map(|n| n as f32).collect();
        self.event(Event::Audio { id, samples });
    }

    fn end(&mut self, id: u64) {
        self.log.push(format!("U{id}"));
        let done = self.done(id);
        self.event(Event::End { id, done });
    }

    fn deliver(&mut self) {
        while let Some((serial, reply)) = self.inbox.pop_front() {
            match reply {
                Some(reply) => self.event(Event::Reply { serial, reply }),
                None => self.event(Event::Gone { serial }),
            }
        }
    }

    fn advance(&mut self, secs: u64) {
        self.now += Duration::from_secs(secs);
        self.event(Event::Tick);
    }

    fn crash(&mut self, keep_replies: bool) {
        let Some((serial, _)) = self.worker.take() else { return };
        if !keep_replies {
            self.inbox.clear();
        }
        self.inbox.push_back((serial, None));
    }

    fn texts(&self) -> HashMap<u64, Result<String, String>> {
        self.results.lock().unwrap().iter().cloned().collect()
    }

    fn count(&self) -> usize {
        self.results.lock().unwrap().len()
    }
}

/// One step of a script.
#[derive(Clone, Debug)]
enum Op {
    Begin(u64),
    Audio(u64, i64, i64),
    End(u64),
    Deliver,
}

fn play(sim: &mut Sim, op: &Op) {
    match op {
        Op::Begin(id) => sim.begin(*id),
        Op::Audio(id, from, to) => sim.audio(*id, *from, *to),
        Op::End(id) => sim.end(*id),
        Op::Deliver => sim.deliver(),
    }
}

/// A script with overlapping dictations, late audio and a late End: the texts it must give.
fn overlapping() -> Vec<Op> {
    use Op::*;
    vec![
        Begin(1),
        Audio(1, 0, 6),
        Deliver,
        Audio(1, 6, 9),
        Begin(2),
        Audio(2, 20, 23),
        Audio(1, 9, 12),
        Deliver,
        End(1),
        Audio(2, 23, 27),
        Deliver,
        End(2),
        Deliver,
    ]
}

/// Runs a script, crashing the worker before step `crash_at` if given, then lets every reply through. Returns what each dictation delivered.
fn run_script(script: &[Op], crash_at: Option<(usize, bool)>) -> (Sim, HashMap<u64, Result<String, String>>) {
    let mut sim = Sim::new();
    for (at, op) in script.iter().enumerate() {
        if let Some((_, keep)) = crash_at.filter(|(point, _)| *point == at) {
            sim.crash(keep);
        }
        play(&mut sim, op);
    }
    if crash_at.is_some_and(|(point, _)| point >= script.len()) {
        sim.crash(true);
    }
    sim.deliver();
    sim.deliver();
    let texts = sim.texts();
    (sim, texts)
}

/// The audit's repro: Begin A, audio A, Begin B, late audio A, End A, End B gives both full texts, and End A goes out before Begin B.
#[test]
fn a_stopped_dictation_is_never_lost_to_the_next_one() {
    let (sim, texts) = run_script(&overlapping(), None);
    assert_eq!(texts[&1], Ok("0-1-2-3 4-5-6-7 8-9-10-11".into()));
    assert_eq!(texts[&2], Ok("20-21-22-23 24-25-26".into()));
    let at = |t: &str| sim.wire.iter().position(|w| w == t).unwrap_or_else(|| panic!("{t} missing in {:?}", sim.wire));
    assert!(at("E1") < at("B2.0"), "{:?}", sim.wire);
}

/// A crash at any point, with the replies in flight kept or lost, gives exactly the text of an uninterrupted run.
#[test]
fn a_crash_at_any_point_gives_the_same_text_as_no_crash() {
    let script = overlapping();
    let (_, expected) = run_script(&script, None);
    for at in 0..=script.len() {
        for keep in [true, false] {
            let (sim, texts) = run_script(&script, Some((at, keep)));
            assert_eq!(texts, expected, "crash before step {at}, replies kept: {keep}; wire {:?}", sim.wire);
        }
    }
}

/// After a crash every dictation goes to the fresh worker in order, and the stopped ones are ended.
#[test]
fn a_crash_replays_stopped_parked_dictations_with_their_end() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.audio(1, 0, 6);
    sim.begin(2);
    sim.audio(2, 10, 12);
    sim.end(2);
    sim.deliver();
    sim.wire.clear();
    sim.crash(false);
    sim.deliver();
    sim.end(1);
    sim.deliver();
    assert_eq!(sim.wire, ["B1.0", "A6", "E1", "B2.0", "A2", "E2"]);
    assert_eq!(sim.texts()[&1], Ok("0-1-2-3 4-5".into()));
    assert_eq!(sim.texts()[&2], Ok("10-11".into()));
}

/// A second crash in a row fails the dictation; text already recognised is kept and reported.
#[test]
fn a_second_crash_fails_but_keeps_the_recognised_text() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.audio(1, 0, 4);
    sim.deliver();
    sim.end(1);
    sim.hold_load = true;
    sim.crash(false);
    sim.deliver();
    assert_eq!(sim.count(), 0, "one retry first");
    sim.crash(false);
    sim.deliver();
    assert_eq!(sim.texts()[&1], Ok("0-1-2-3".into()));
    assert!(sim.reports.iter().any(|r| r.contains("kept")), "{:?}", sim.reports);
    assert_eq!(sim.spawns.len(), 2);
}

/// With nothing recognised a double crash is an error, and a later dictation gets a fresh start and its own retry.
#[test]
fn a_crash_with_nothing_recognised_is_an_error_and_the_next_dictation_starts_over() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.deliver();
    sim.end(1);
    sim.hold_load = true;
    sim.crash(false);
    sim.deliver();
    sim.crash(false);
    sim.deliver();
    assert!(sim.texts()[&1].is_err());
    sim.hold_load = false;
    sim.begin(2);
    sim.deliver();
    sim.audio(2, 0, 4);
    sim.crash(false);
    sim.deliver();
    sim.end(2);
    sim.deliver();
    assert_eq!(sim.texts()[&2], Ok("0-1-2-3".into()));
}

/// A worker that never finishes loading fails the waiting dictation at the load limit.
#[test]
fn a_load_that_takes_too_long_fails_the_dictation() {
    let mut sim = Sim::new();
    sim.hold_load = true;
    sim.begin(1);
    sim.audio(1, 0, 4);
    sim.end(1);
    sim.advance(179);
    assert_eq!(sim.count(), 0);
    sim.advance(2);
    assert_eq!(sim.texts()[&1], Err("The speech model took too long to load.".into()));
    assert_eq!(sim.kills, 1);
}

/// After End the worker has at least the run floor, and a new End does not push the hang check further out.
#[test]
fn the_run_deadline_is_not_rearmed_by_each_end() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.deliver();
    sim.mute = true;
    sim.end(1);
    let first = sim.session.deadline().expect("armed");
    sim.advance(20);
    sim.begin(2);
    sim.end(2);
    assert_eq!(sim.session.deadline(), Some(first));
    sim.advance(11);
    assert_eq!(sim.texts()[&1], Err("Transcription took too long and was stopped.".into()));
    assert!(sim.texts()[&2].is_err());
}

/// A worker that stops answering while the user records is detected, and any reply restarts the clock.
#[test]
fn a_stalled_worker_during_a_recording_is_detected_and_a_reply_resets_it() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.deliver();
    sim.mute = true;
    sim.audio(1, 0, 4);
    let armed = sim.session.deadline().expect("armed");
    assert_eq!(armed - sim.now, Duration::from_secs(120));
    sim.advance(100);
    let serial = sim.worker.as_ref().map(|(s, _)| *s).expect("worker");
    sim.event(Event::Reply { serial, reply: Reply::Release { id: 1, up_to: 0, clauses: 0 } });
    sim.audio(1, 4, 8);
    assert_eq!(sim.session.deadline(), Some(sim.now + Duration::from_secs(120)));
    sim.advance(121);
    sim.end(1);
    assert!(sim.texts()[&1].is_err());
    assert_eq!(sim.kills, 1);
}

/// Escape drops the recording by its id, and nothing of it reaches a worker that is still loading; earlier dictations still go out.
#[test]
fn cancel_truncates_only_its_own_backlog() {
    let mut sim = Sim::new();
    sim.hold_load = true;
    sim.begin(1);
    sim.audio(1, 0, 4);
    sim.end(1);
    sim.begin(2);
    sim.audio(2, 10, 14);
    sim.event(Event::Cancel { id: 3 });
    sim.event(Event::Cancel { id: 2 });
    sim.event(Event::Reply { serial: 1, reply: Reply::Loaded { ms: 1, version: VERSION } });
    assert_eq!(sim.wire, ["B1.0", "A4", "E1"]);
    sim.deliver();
    assert_eq!(sim.texts()[&1], Ok("0-1-2-3".into()));
    assert!(!sim.texts().contains_key(&2));
}

/// A cancelled recording that already reached a loaded worker is dropped without waiting; the next dictation goes straight out.
#[test]
fn cancel_frees_the_wire_for_the_next_dictation() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.deliver();
    sim.audio(1, 0, 4);
    sim.event(Event::Cancel { id: 1 });
    sim.begin(2);
    sim.audio(2, 5, 9);
    sim.end(2);
    sim.deliver();
    assert_eq!(sim.texts()[&2], Ok("5-6-7-8".into()));
    assert!(sim.partials.iter().all(|(id, _)| *id != 1), "{:?}", sim.partials);
}

/// The worker leaves after the user's idle time, never while a dictation is open, and "Immediately" leaves at once.
#[test]
fn the_idle_unload_follows_the_setting() {
    let mut sim = Sim::new();
    sim.session.set_unload(Unload::After(120));
    sim.begin(1);
    sim.deliver();
    sim.audio(1, 0, 4);
    sim.deliver();
    sim.advance(1000);
    assert_eq!(sim.kills, 0, "an open recording holds the model");
    sim.end(1);
    sim.deliver();
    assert_eq!(sim.session.deadline(), Some(sim.now + Duration::from_secs(120)));
    sim.advance(119);
    assert_eq!(sim.kills, 0);
    sim.advance(2);
    assert_eq!(sim.kills, 1);
    assert!(sim.session.quiescent());
    assert_eq!(sim.session.deadline(), None);

    let mut now = Sim::new();
    now.session.set_unload(Unload::Immediately);
    now.begin(1);
    now.deliver();
    now.end(1);
    now.deliver();
    assert_eq!(now.kills, 1);

    let mut never = Sim::new();
    never.session.set_unload(Unload::Never);
    never.begin(1);
    never.deliver();
    never.end(1);
    never.deliver();
    assert_eq!(never.session.deadline(), None);
}

/// A recording nobody ended is dropped after its cap plus margin so it cannot hold the model for ever, and the idle clock then starts.
#[test]
fn a_stuck_recording_is_dropped_at_the_cap_and_the_idle_unload_resumes() {
    let mut sim = Sim::new();
    sim.session.set_unload(Unload::After(60));
    sim.begin(1);
    sim.deliver();
    sim.advance(1229);
    assert!(!sim.session.quiescent());
    sim.advance(2);
    assert!(sim.reports.iter().any(|r| r.contains("ran too long")), "{:?}", sim.reports);
    sim.end(1);
    assert!(sim.texts()[&1].is_err(), "a late End is answered, not lost");
    assert_eq!(sim.session.deadline(), Some(sim.now + Duration::from_secs(60)));
}

/// A recording replaced by a newer one that goes quiet is dropped, and the newer one then goes out.
#[test]
fn a_superseded_silent_recording_is_dropped_and_the_next_goes_out() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.deliver();
    sim.begin(2);
    sim.audio(2, 0, 4);
    assert_eq!(sim.wire, ["B1.0"]);
    sim.advance(21);
    assert_eq!(sim.wire, ["B1.0", "B2.0", "A4"]);
    sim.end(2);
    sim.deliver();
    assert_eq!(sim.texts()[&2], Ok("0-1-2-3".into()));
}

/// A model switch waits for the dictations still in flight (they keep their worker) and applies once they are done.
#[test]
fn a_model_switch_waits_for_dictations_in_flight() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.deliver();
    sim.audio(1, 0, 4);
    sim.begin_with(2, Ok(PICK_B));
    assert_eq!((sim.spawns.len(), sim.kills), (1, 0), "dictation 1 keeps its worker");
    sim.end(1);
    sim.end(2);
    sim.deliver();
    assert_eq!(sim.texts()[&1], Ok("0-1-2-3".into()));
    sim.begin_with(3, Ok(PICK_B));
    assert_eq!((sim.spawns.clone(), sim.kills), (vec![PICK_A, PICK_B], 1));
}

/// Unload ends every dictation: stopped ones get what was recognised, and the worker is gone.
#[test]
fn unload_delivers_stopped_dictations_and_kills_the_worker() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.deliver();
    sim.audio(1, 0, 4);
    sim.deliver();
    sim.mute = true;
    sim.end(1);
    sim.event(Event::Unload);
    assert_eq!(sim.texts()[&1], Ok("0-1-2-3".into()));
    assert!(sim.session.quiescent());
}

/// A worker whose queue is full fails the dictations but keeps their text; a worker of another protocol version is refused.
#[test]
fn overload_and_a_version_mismatch_fail_and_keep_the_text() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.deliver();
    sim.audio(1, 0, 4);
    sim.deliver();
    sim.end(1);
    let serial = sim.worker.as_ref().map(|(s, _)| *s).expect("worker");
    sim.event(Event::Overload { serial });
    assert_eq!(sim.texts()[&1], Ok("0-1-2-3".into()));

    let mut old = Sim::new();
    old.hold_load = true;
    old.begin(1);
    old.event(Event::Reply { serial: 1, reply: Reply::Loaded { ms: 1, version: VERSION + 1 } });
    old.end(1);
    assert!(old.texts()[&1].is_err());
}

/// No model and a worker that cannot start end the dictation with an error, not a hang.
#[test]
fn a_missing_model_or_a_failed_start_ends_with_an_error() {
    let mut sim = Sim::new();
    sim.begin_with(1, Err("No speech model installed.".into()));
    sim.audio(1, 0, 4);
    sim.end(1);
    assert_eq!(sim.texts()[&1], Err("No speech model installed.".into()));
    assert!(sim.reports.is_empty(), "the dictation's error is the only notice: {:?}", sim.reports);
    assert!(sim.session.quiescent());

    let mut broken = Sim::new();
    broken.begin(1);
    broken.event(Event::SpawnFailed { serial: 1, message: "cannot start".into() });
    broken.end(1);
    assert_eq!(broken.texts()[&1], Err("cannot start".into()));
    assert!(broken.reports.is_empty(), "the dictation's error is the only notice: {:?}", broken.reports);
}

/// An End for a recording nobody knows is answered once with an error.
#[test]
fn an_unknown_end_is_answered_once() {
    let mut sim = Sim::new();
    sim.end(9);
    sim.end(9);
    assert_eq!(sim.count(), 2);
    assert!(sim.texts()[&9].is_err());
}

#[derive(Clone, Debug)]
enum Step {
    Begin,
    Audio(usize),
    End(usize),
    Cancel(usize),
    Crash(bool),
    Deliver,
    Advance(u64),
    Unload,
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    let step = prop_oneof![
        3 => Just(Step::Begin),
        6 => (0usize..8).prop_map(Step::Audio),
        3 => (0usize..8).prop_map(Step::End),
        1 => (0usize..8).prop_map(Step::Cancel),
        1 => any::<bool>().prop_map(Step::Crash),
        6 => Just(Step::Deliver),
        2 => (0u64..200).prop_map(Step::Advance),
        1 => Just(Step::Unload),
    ];
    proptest::collection::vec(step, 1..60)
}

/// Dictation ids the property test has opened, by what became of them.
#[derive(Default)]
struct Book {
    next: u64,
    open: Vec<u64>,
    ended: HashSet<u64>,
    cancelled: HashSet<u64>,
}

impl Book {
    fn pick(&self, n: usize) -> Option<u64> {
        (!self.open.is_empty()).then(|| self.open[n % self.open.len()])
    }
}

fn apply(sim: &mut Sim, book: &mut Book, step: &Step) {
    match step {
        Step::Begin => {
            book.next += 1;
            book.open.push(book.next);
            sim.begin(book.next);
        }
        Step::Audio(n) => {
            if let Some(id) = book.pick(*n) {
                sim.audio(id, 0, 3);
            }
        }
        Step::End(n) => {
            if let Some(id) = book.pick(*n) {
                book.open.retain(|o| *o != id);
                book.ended.insert(id);
                sim.end(id);
            }
        }
        Step::Cancel(n) => {
            if let Some(id) = book.pick(*n) {
                book.open.retain(|o| *o != id);
                book.cancelled.insert(id);
                sim.event(Event::Cancel { id });
                sim.partials.retain(|(p, _)| *p != id);
            }
        }
        Step::Crash(keep) => sim.crash(*keep),
        Step::Deliver => sim.deliver(),
        Step::Advance(secs) => sim.advance(*secs),
        Step::Unload => sim.event(Event::Unload),
    }
}

proptest! {
    /// Over any mix of events: every ended dictation's callback runs exactly once (after an unload at the end), a cancelled one never gets
    /// text, and a dictation that was ended never has its Begin overtaken: End A is on the wire before the Begin of a later dictation.
    #[test]
    fn every_callback_fires_once_and_nothing_survives_a_cancel(steps in steps()) {
        let mut sim = Sim::new();
        let mut book = Book::default();
        for step in &steps {
            apply(&mut sim, &mut book, step);
        }
        sim.event(Event::Unload);
        sim.advance(100_000);
        let results = sim.results.lock().unwrap().clone();
        for id in &book.ended {
            prop_assert_eq!(results.iter().filter(|(r, _)| r == id).count(), 1, "dictation {} in {:?}", id, steps);
        }
        for id in &book.cancelled {
            prop_assert!(results.iter().all(|(r, _)| r != id));
        }
        prop_assert!(sim.session.quiescent());
        prop_assert_eq!(sim.session.deadline(), None);
        let clean = !steps.iter().any(|s| matches!(s, Step::Advance(_))) && steps.iter().filter(|s| matches!(s, Step::Begin)).count() < 4;
        prop_assert!(!clean || order_holds(&sim.log), "{:?}", sim.log);
    }
}

/// In the log (a worker's start is marked `S`, a user's End `U<id>`), a Begin never goes out while an earlier dictation whose End the session
/// already had is still open on the wire.
fn order_holds(log: &[String]) -> bool {
    let (mut open, mut stopped) = (None::<String>, HashSet::new());
    for (item, id) in log.iter().map(|item| (item.as_bytes().first().copied(), item.get(1..).unwrap_or("").split('.').next().unwrap_or("")))
    {
        match item {
            Some(b'S') => open = None,
            Some(b'U') => {
                stopped.insert(id.to_string());
            }
            Some(b'B') => {
                if open.as_ref().is_some_and(|prev| prev != id && stopped.contains(prev)) {
                    return false;
                }
                open = Some(id.to_string());
            }
            Some(b'E') if open.as_deref() == Some(id) => open = None,
            _ => {}
        }
    }
    true
}

/// Load time and stop-to-text are logged per model, with no text in the line.
#[test]
fn load_and_stop_to_text_times_are_logged_per_model() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.audio(1, 0, 4);
    sim.deliver();
    sim.end(1);
    sim.now += Duration::from_millis(250);
    sim.deliver();
    assert!(sim.logs.iter().any(|l| l == "speech: a (Segmented(Transducer)) loaded in 1 ms"), "{:?}", sim.logs);
    assert!(sim.logs.iter().any(|l| l == "speech: a text 250 ms after stop"), "{:?}", sim.logs);
}

/// Clauses found while their audio is still unreleased are not numbered into the replay: the crash replays that audio under the released count,
/// so each clause is typed once.
#[test]
fn a_clause_committed_before_its_audio_is_released_is_not_repeated() {
    let mut sim = Sim::new();
    sim.begin(1);
    sim.audio(1, 0, 6);
    sim.deliver();
    sim.crash(true);
    sim.deliver();
    sim.audio(1, 6, 8);
    sim.end(1);
    sim.deliver();
    assert_eq!(sim.texts()[&1], Ok("0-1-2-3 4-5-6-7".into()));
    assert!(sim.wire.contains(&"B1.0".to_string()) && sim.wire.iter().filter(|w| *w == "B1.0").count() == 2, "{:?}", sim.wire);
}
