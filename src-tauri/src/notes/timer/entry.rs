//! File: notes/timer/entry.rs
//! Purpose: One note's running timer: its clock (start, pause, restart, phase changes) and what the page draws of it.
//! Contents: Phase — run, focus or rest; Entry — spec, owner, phase, start and pause instants, with due / advance / toggle_pause / restart / view; Said — what a phase change announces; View — the page's picture of a timer.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::super::breaks::Owner;
use super::spec::{Kind, Spec};

/// Where a timer is: a countdown or stopwatch runs, a pomodoro focuses or rests.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Run,
    Focus,
    Rest,
}

/// One note's running timer.
pub struct Entry {
    pub spec: Spec,
    /// Its note and generation: a new or restarted timer is a new generation, so what the old one queued or opened is told apart.
    pub owner: Owner,
    /// The line as typed (trimmed): the same line leaves a running timer alone, another one restarts it.
    pub line: String,
    pub phase: Phase,
    /// When the current phase began; moved forward by the time spent paused.
    began: Instant,
    pub paused_at: Option<Instant>,
    /// Focus periods completed (pomodoro).
    rounds: u32,
}

/// A phase change to announce.
pub struct Said {
    /// The timer (note and generation) that said it.
    pub owner: Owner,
    pub text: String,
    pub kind: Kind,
    /// The phase that has begun.
    pub phase: Phase,
    pub rest_secs: u64,
    pub rounds: u32,
    /// A countdown has run out: the timer is over.
    pub done: bool,
}

/// What the page draws: the timer, its phase, how far into the phase it is and how long the phase lasts (0 for a stopwatch). `toast` names what a command just changed.
#[derive(Serialize)]
pub struct View {
    kind: &'static str,
    phase: &'static str,
    name: String,
    elapsed_ms: u64,
    total_ms: u64,
    rounds: u32,
    /// The pomodoro's set number of rounds, 0 for no end.
    of: u32,
    paused: bool,
    toast: Option<String>,
}

impl View {
    /// This view with a toast naming what a command just changed.
    pub fn with_toast(self, toast: impl Into<String>) -> Self {
        View { toast: Some(toast.into()), ..self }
    }
}

/// The phase a timer starts and restarts in.
fn first_phase(spec: &Spec) -> Phase {
    if spec.kind == Kind::Pomodoro {
        Phase::Focus
    } else {
        Phase::Run
    }
}

/// Whole minutes, or one decimal: `25`, `0.2`.
fn minutes(secs: u64) -> f64 {
    (secs as f64 / 6.0).round() / 10.0
}

/// The next generation: unique for the life of the process.
fn next_generation() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl Entry {
    /// A timer of note `note` that begins now. Inputs: the note id, the spec and the line as typed.
    pub fn new(note: i64, spec: Spec, line: String) -> Self {
        let owner = Owner { note, generation: next_generation() };
        Entry { phase: first_phase(&spec), spec, owner, line, began: Instant::now(), paused_at: None, rounds: 0 }
    }

    /// The toast that says the timer began.
    pub fn started(&self) -> String {
        match self.spec.kind {
            Kind::Stopwatch => "Stopwatch started".into(),
            Kind::Countdown => "Countdown started".into(),
            Kind::Pomodoro => format!("{}/{} Pomodoro started", minutes(self.spec.secs), minutes(self.spec.rest)),
        }
    }

    /// How long the current phase lasts; None for a stopwatch.
    fn length(&self) -> Option<Duration> {
        match self.phase {
            Phase::Focus | Phase::Run if self.spec.kind != Kind::Stopwatch => Some(Duration::from_secs(self.spec.secs)),
            Phase::Rest => Some(Duration::from_secs(self.spec.rest)),
            _ => None,
        }
    }

    /// When the phase ends; a paused timer has no deadline.
    pub fn due(&self) -> Option<Instant> {
        if self.paused_at.is_some() {
            return None;
        }
        self.length().map(|l| self.began + l)
    }

    /// The phase ended: the next one begins from the deadline, not from now, so a late wake-up does not stretch it. A countdown is done, a pomodoro goes from focus to rest and back.
    pub fn advance(&mut self) -> Said {
        self.began = self.due().unwrap_or(self.began);
        let name = if self.spec.name.is_empty() { String::new() } else { format!(": {}", self.spec.name) };
        let (text, done) = match self.phase {
            Phase::Focus => {
                self.rounds += 1;
                let last = self.spec.rounds > 0 && self.rounds >= self.spec.rounds;
                self.phase = if last { Phase::Focus } else { Phase::Rest };
                if last {
                    ("All rounds done", true)
                } else {
                    ("Focus done, time to rest", false)
                }
            }
            Phase::Rest => {
                self.phase = Phase::Focus;
                ("Rest over, back to focus", false)
            }
            Phase::Run => ("Timer done", true),
        };
        let (kind, phase, rest_secs, rounds) = (self.spec.kind, self.phase, self.spec.rest, self.rounds);
        Said { owner: self.owner, text: format!("{text}{name}"), kind, phase, rest_secs, rounds, done }
    }

    /// Pauses, or resumes by moving the start forward by the time spent paused. Returns the toast.
    pub fn toggle_pause(&mut self) -> &'static str {
        match self.paused_at.take() {
            Some(at) => {
                self.began += at.elapsed();
                "Timer resumed"
            }
            None => {
                self.paused_at = Some(Instant::now());
                "Timer paused"
            }
        }
    }

    /// Starts over as a new generation: rounds and phase reset, pause cleared.
    pub fn restart(&mut self) {
        self.owner.generation = next_generation();
        (self.began, self.paused_at, self.rounds, self.phase) = (Instant::now(), None, 0, first_phase(&self.spec));
    }

    /// What the page draws of the timer now.
    pub fn view(&self) -> View {
        let elapsed = self.paused_at.unwrap_or_else(Instant::now).saturating_duration_since(self.began);
        let kind = match self.spec.kind {
            Kind::Stopwatch => "stopwatch",
            Kind::Countdown => "countdown",
            Kind::Pomodoro => "pomodoro",
        };
        let phase = match self.phase {
            Phase::Run => "run",
            Phase::Focus => "focus",
            Phase::Rest => "rest",
        };
        View {
            kind,
            phase,
            name: self.spec.name.clone(),
            elapsed_ms: elapsed.as_millis() as u64,
            total_ms: self.length().map_or(0, |l| l.as_millis() as u64),
            rounds: self.rounds,
            of: self.spec.rounds,
            paused: self.paused_at.is_some(),
            toast: None,
        }
    }
}
