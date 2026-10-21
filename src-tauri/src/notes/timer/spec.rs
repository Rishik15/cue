//! Purpose: Reads a timer line: `timer` alone is a stopwatch, `timer 5` (minutes, `3.5` and `3:30` allowed) or `timer 1h30m` counts down, `timer until 3:30pm` or `timer 9AM` counts down to a clock time, `timer pomodoro` (25/5) or `timer 5 1` (5 minutes of focus, 1 of rest) alternates focus and rest, forever or `x4` times. `timer 5: Laundry` names it (the title follows the first colon not followed by a digit); other words are the name too. `timer p`, `r`, `s` are controls.
//! Contents: Spec / Kind — what a line asks for; Control / control — the pause, restart and stop words; parse — line to Spec, or None when the line is not a timer or asks for nothing.

use super::clock::{clock_time, until, Clock};
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Stopwatch,
    Countdown,
    Pomodoro,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Spec {
    pub kind: Kind,
    pub name: String,
    /// The countdown, or the focus time of a pomodoro, in seconds.
    pub secs: u64,
    /// The rest between two focus times of a pomodoro, in seconds.
    pub rest: u64,
    /// How many focus times a pomodoro runs (`x4`); 0 for no end.
    pub rounds: u32,
}

/// What a control line or the page's buttons ask of the running timer (`pause` toggles).
#[derive(Clone, Copy, PartialEq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Control {
    Pause,
    Restart,
    Stop,
}

const POMODORO: (u64, u64) = (25 * 60, 5 * 60);
/// The longest timer: a day (a day-long countdown is a calendar's job).
const LONGEST: u64 = 24 * 3600;

/// `timer p`, `timer r`, `timer s` / `timer 0`: one control word and nothing else.
pub fn control(line: &str) -> Option<Control> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("timer") {
        return None;
    }
    let arg = words.next()?;
    if words.next().is_some() {
        return None;
    }
    match arg.to_lowercase().as_str() {
        "p" => Some(Control::Pause),
        "r" => Some(Control::Restart),
        "s" | "0" => Some(Control::Stop),
        _ => None,
    }
}

#[derive(Default)]
struct Found {
    work: Option<u64>,
    rest: Option<u64>,
    pomodoro: bool,
    rounds: u32,
    name: Vec<String>,
}

pub fn parse(line: &str, now: Clock) -> Option<Spec> {
    if control(line).is_some() {
        return None;
    }
    let (args, title) = split_title(line.trim());
    let mut words = args.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("timer") {
        return None;
    }
    let words = join_units(words.collect());
    let (mut found, mut i) = (Found::default(), 0);
    while i < words.len() {
        i += found.take(&words, i, now)?;
    }
    found.finish(title)
}

impl Found {
    /// Reads the word at `i`; returns how many words it used, None for a clock time that is not one.
    fn take(&mut self, words: &[String], i: usize, now: Clock) -> Option<usize> {
        let lower = words[i].to_lowercase();
        if matches!(lower.as_str(), "pomodoro" | "pomo") {
            self.pomodoro = true;
            return Some(1);
        }
        if let Some(n) = rounds(&lower) {
            self.rounds = n;
            return Some(1);
        }
        let from = if lower == "until" { i + 1 } else { i };
        if from > i || meridiem(words, i) {
            let (clock, used) = clock_time(&words[from..])?;
            self.work = Some(until(clock, now));
            return Some(used + from - i);
        }
        if let Some(s) = duration(&lower).or_else(|| minutes(&lower)) {
            // A bare number is minutes; the second time (`5s 2s`, `25 5`) is the rest of a pomodoro.
            match self.work {
                None => self.work = Some(s),
                Some(_) => (self.rest, self.pomodoro) = (Some(s), true),
            }
        } else {
            self.name.push(words[i].clone());
        }
        Some(1)
    }

    fn finish(self, title: &str) -> Option<Spec> {
        let name = if title.is_empty() { self.name.join(" ") } else { title.to_string() };
        let rest = self.rest.unwrap_or(POMODORO.1);
        if self.work > Some(LONGEST) || self.work == Some(0) || rest == 0 || rest > LONGEST {
            return None;
        }
        match (self.pomodoro, self.work) {
            (true, work) => Some(Spec { kind: Kind::Pomodoro, name, secs: work.unwrap_or(POMODORO.0), rest, rounds: self.rounds }),
            (false, Some(s)) => Some(Spec { kind: Kind::Countdown, name, secs: s, rest: 0, rounds: 0 }),
            (false, None) => Some(Spec { kind: Kind::Stopwatch, name, secs: 0, rest: 0, rounds: 0 }),
        }
    }
}

/// `x4` or `4x`: how many focus times a pomodoro runs (1 to 99).
fn rounds(word: &str) -> Option<u32> {
    let digits = word.strip_prefix('x').or_else(|| word.strip_suffix('x'))?;
    digits.parse().ok().filter(|n| (1..=99).contains(n))
}

/// Splits at the first colon not followed by a digit (`3:30` stays a duration): the arguments and the title.
fn split_title(line: &str) -> (&str, &str) {
    let at = line.char_indices().find(|&(i, c)| c == ':' && !line[i + 1..].starts_with(|n: char| n.is_ascii_digit()));
    at.map_or((line, ""), |(i, _)| (&line[..i], line[i + 1..].trim()))
}

/// A clock time with am/pm: `9am`, `9:30pm`, or `9` followed by `am`. Plain `21:15` is not one: it is 21 minutes 15 seconds, and only `until` reads a 24-hour clock.
fn meridiem(words: &[String], i: usize) -> bool {
    let w = words[i].to_lowercase();
    let next = words.get(i + 1).map(|n| n.to_lowercase());
    w.starts_with(|c: char| c.is_ascii_digit()) && (w.ends_with("am") || w.ends_with("pm") || matches!(next.as_deref(), Some("am" | "pm")))
}

/// `5` or `3.5` minutes, or `3:30` (minutes and seconds), in seconds.
fn minutes(word: &str) -> Option<u64> {
    if let Some((m, s)) = word.split_once(':') {
        let digits = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
        let (m, s): (u64, u64) = (m.parse().ok().filter(|_| digits(m))?, s.parse().ok().filter(|_| digits(s))?);
        return (s < 60).then_some(m * 60 + s);
    }
    if !word.starts_with(|c: char| c.is_ascii_digit() || c == '.') || !word.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    Some((word.parse::<f64>().ok()? * 60.0).round() as u64)
}

/// "5 min" is one word, "5min".
fn join_units(words: Vec<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in words {
        let unit = w.chars().all(char::is_alphabetic);
        match out.last_mut() {
            Some(last)
                if unit
                    && last.chars().last().is_some_and(|c| c.is_ascii_digit())
                    && !w.eq_ignore_ascii_case("am")
                    && !w.eq_ignore_ascii_case("pm") =>
            {
                last.push_str(w)
            }
            _ => out.push(w.to_string()),
        }
    }
    out
}

/// `5m`, `90s`, `1h30m`, `1.5h`: pairs of a number and a unit. None for anything else.
fn duration(word: &str) -> Option<u64> {
    let (mut total, mut rest) = (0.0, word);
    while !rest.is_empty() {
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
        let number: f64 = rest[..end].parse().ok()?;
        let unit_len = rest[end..].find(|c: char| c.is_ascii_digit() || c == '.').unwrap_or(rest.len() - end);
        let unit = match &rest[end..end + unit_len] {
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
            "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
            "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
            _ => return None,
        };
        total += number * unit;
        rest = &rest[end + unit_len..];
    }
    Some(total.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: Clock = Clock { hour: 12, minute: 0, second: 0 };

    fn p(line: &str) -> Option<(Kind, String, u64, u64)> {
        parse(line, NOON).map(|s| (s.kind, s.name, s.secs, s.rest))
    }

    #[test]
    fn rounds_are_read_as_x_or_times() {
        let rounds = |l: &str| parse(l, NOON).map(|s| s.rounds);
        assert_eq!(rounds("timer pomodoro x4"), Some(4));
        assert_eq!(rounds("timer 25 5 4x"), Some(4));
        assert_eq!(rounds("timer pomodoro"), Some(0));
        assert_eq!(rounds("timer pomodoro x0"), Some(0)); // not a count: a name word
    }

    #[test]
    fn kinds_and_durations() {
        assert_eq!(p("timer"), Some((Kind::Stopwatch, String::new(), 0, 0)));
        assert_eq!(p("Timer tea 3m"), Some((Kind::Countdown, "tea".into(), 180, 0)));
        assert_eq!(p("timer 1h30m"), Some((Kind::Countdown, String::new(), 5400, 0)));
        assert_eq!(p("timer 5 min eggs"), Some((Kind::Countdown, "eggs".into(), 300, 0)));
        assert_eq!(p("timer 1.5h"), Some((Kind::Countdown, String::new(), 5400, 0)));
        assert_eq!(p("timer pomodoro"), Some((Kind::Pomodoro, String::new(), 1500, 300)));
        assert_eq!(p("timer pomodoro 50m"), Some((Kind::Pomodoro, String::new(), 3000, 300)));
    }

    #[test]
    fn bare_numbers_are_minutes() {
        assert_eq!(p("timer 5"), Some((Kind::Countdown, String::new(), 300, 0)));
        assert_eq!(p("timer 3.5"), Some((Kind::Countdown, String::new(), 210, 0)));
        assert_eq!(p("timer 3:30"), Some((Kind::Countdown, String::new(), 210, 0)));
        assert_eq!(p("timer 21:15"), Some((Kind::Countdown, String::new(), 1275, 0)));
        assert_eq!(p("timer 3:75"), Some((Kind::Stopwatch, "3:75".into(), 0, 0))); // not a duration: a name
        assert_eq!(p("timer 5 1"), Some((Kind::Pomodoro, String::new(), 300, 60)));
        assert_eq!(p("timer 0.2 0.1"), Some((Kind::Pomodoro, String::new(), 12, 6)));
        assert_eq!(p("timer 5 0"), None);
    }

    #[test]
    fn a_second_time_with_a_unit_is_the_rest() {
        assert_eq!(p("timer 5s 2s x4"), Some((Kind::Pomodoro, String::new(), 5, 2)));
        assert_eq!(p("timer 25m 5m"), Some((Kind::Pomodoro, String::new(), 1500, 300)));
        assert_eq!(p("timer 25m 5"), Some((Kind::Pomodoro, String::new(), 1500, 300)));
    }

    #[test]
    fn titles_follow_the_first_colon_without_a_digit() {
        assert_eq!(p("timer 5: Do laundry"), Some((Kind::Countdown, "Do laundry".into(), 300, 0)));
        assert_eq!(p("timer 5: Errands: bank, post office"), Some((Kind::Countdown, "Errands: bank, post office".into(), 300, 0)));
        assert_eq!(p("timer 3:30: tea"), Some((Kind::Countdown, "tea".into(), 210, 0)));
    }

    #[test]
    fn clock_times_count_to_the_next_occurrence() {
        assert_eq!(p("timer until 3pm"), Some((Kind::Countdown, String::new(), 3 * 3600, 0)));
        assert_eq!(p("timer until 3:30 pm"), Some((Kind::Countdown, String::new(), 3 * 3600 + 1800, 0)));
        assert_eq!(p("timer until 11:00").map(|s| s.2), Some(23 * 3600)); // already past today
        assert_eq!(p("timer until 12:00").map(|s| s.2), Some(86_400));
        assert_eq!(p("timer until 13pm"), None);
        assert_eq!(p("timer 9AM").map(|s| s.2), Some(21 * 3600));
        assert_eq!(p("timer 9:30pm").map(|s| s.2), Some(9 * 3600 + 1800));
        assert_eq!(p("timer 1 pm").map(|s| s.2), Some(3600));
    }

    #[test]
    fn controls() {
        assert_eq!(control("timer p"), Some(Control::Pause));
        assert_eq!(control("Timer R"), Some(Control::Restart));
        assert_eq!(control("timer s"), Some(Control::Stop));
        assert_eq!(control("timer 0"), Some(Control::Stop));
        assert_eq!(control("timer p now"), None);
        assert_eq!(control("timer 5"), None);
        assert_eq!(p("timer p"), None);
    }

    #[test]
    fn not_a_timer() {
        assert_eq!(p("timers"), None);
        assert_eq!(p("math"), None);
        assert_eq!(p("timer 0m"), None);
        assert_eq!(p("timer 99h"), None);
    }
}
