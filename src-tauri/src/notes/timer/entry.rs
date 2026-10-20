//! Purpose: One note's running timer: its clock (start, pause, restart, phase changes) and what the page draws of it.
//! Contents: Phase — run, focus or rest; Entry — spec, phase, start and pause instants, with due / advance / toggle_pause / restart / view; Said — what a phase change announces; View — the page's picture of a timer.

use std::time::{Duration, Instant};

use serde::Serialize;

use super::spec::{Kind, Spec};

#[derive(Clone, Copy, PartialEq)]
pub enum Phase {
    Run,
    Focus,
    Rest,
}

pub struct Entry {
    pub spec: Spec,
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
    pub fn with_toast(self, toast: impl Into<String>) -> Self {
        View { toast: Some(toast.into()), ..self }
    }
}

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

impl Entry {
    pub fn new(spec: Spec, line: String) -> Self {
        Entry { phase: first_phase(&spec), spec, line, began: Instant::now(), paused_at: None, rounds: 0 }
    }

    pub fn started(&self) -> String {
        match self.spec.kind {
            Kind::Stopwatch => "Stopwatch started".into(),
            Kind::Countdown => "Countdown started".into(),
            Kind::Pomodoro => format!("{}/{} Pomodoro started", minutes(self.spec.secs), minutes(self.spec.rest)),
        }
    }

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

    /// The phase ended: a countdown is done, a pomodoro goes from focus to rest and back.
    pub fn advance(&mut self) -> Said {
        self.began = self.due().unwrap_or(self.began); // from the deadline, not from now, so a late wake-up does not stretch the next phase
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
        Said { text: format!("{text}{name}"), kind, phase, rest_secs, rounds, done }
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

    pub fn restart(&mut self) {
        (self.began, self.paused_at, self.rounds, self.phase) = (Instant::now(), None, 0, first_phase(&self.spec));
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn pomodoro() -> Entry {
        Entry::new(Spec { kind: Kind::Pomodoro, name: String::new(), secs: 12, rest: 6, rounds: 0 }, "timer 0.2 0.1".into())
    }

    #[test]
    fn focus_ends_in_rest_and_counts_a_round() {
        let mut e = pomodoro();
        let said = e.advance();
        assert!(said.phase == Phase::Rest && said.rounds == 1 && !said.done);
        assert_eq!(e.advance().text, "Rest over, back to focus");
    }

    #[test]
    fn a_set_number_of_rounds_ends_the_pomodoro_after_the_last_focus() {
        let mut e = Entry::new(Spec { kind: Kind::Pomodoro, name: String::new(), secs: 12, rest: 6, rounds: 2 }, String::new());
        assert!(!e.advance().done); // focus 1
        e.advance(); // rest
        let last = e.advance(); // focus 2
        assert!(last.done && last.rounds == 2 && last.phase == Phase::Focus);
    }

    #[test]
    fn pause_has_no_deadline_and_resume_moves_it_later() {
        let mut e = pomodoro();
        let due = e.due();
        e.paused_at = Some(Instant::now() - Duration::from_secs(5));
        assert!(e.due().is_none() && e.view().paused);
        assert_eq!(e.toggle_pause(), "Timer resumed");
        assert!(e.due() >= due.map(|d| d + Duration::from_secs(5)));
    }

    #[test]
    fn restart_resets_rounds_and_phase() {
        let mut e = pomodoro();
        e.advance();
        e.restart();
        assert!(e.phase == Phase::Focus && e.view().rounds == 0);
    }

    #[test]
    fn started_names_the_real_minutes() {
        assert_eq!(pomodoro().started(), "0.2/0.1 Pomodoro started");
        let e = Entry::new(Spec { kind: Kind::Pomodoro, name: String::new(), secs: 1500, rest: 300, rounds: 0 }, String::new());
        assert_eq!(e.started(), "25/5 Pomodoro started");
    }
}
