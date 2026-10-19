//! Purpose: Reads the first line of a timer note: `timer` alone is a stopwatch, `timer 5m` or `timer 1h30m` counts down, `timer until 3:30pm` counts down to a clock time, `timer pomodoro` alternates 25 minutes of focus and 5 of rest (`timer pomodoro 50m` changes the focus). Other words are the timer's name.
//! Contents: Spec / Kind — what a line asks for; Clock — the time of day `until` counts from; parse — line to Spec, or None when the line is not a timer or asks for nothing.

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
}

/// The time of day, for `until`.
#[derive(Clone, Copy)]
pub struct Clock {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

const POMODORO: (u64, u64) = (25 * 60, 5 * 60);
/// The longest timer: a day (a day-long countdown is a calendar's job).
const LONGEST: u64 = 24 * 3600;

pub fn parse(line: &str, now: Clock) -> Option<Spec> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("timer") {
        return None;
    }
    let words = join_units(words.collect());
    let (mut secs, mut pomodoro, mut name) = (None, false, Vec::new());
    let mut i = 0;
    while i < words.len() {
        let lower = words[i].to_lowercase();
        if matches!(lower.as_str(), "pomodoro" | "pomo") {
            pomodoro = true;
        } else if lower == "until" {
            let (clock, used) = clock_time(&words[i + 1..])?;
            secs = Some(until(clock, now));
            i += used;
        } else if let Some(s) = duration(&lower) {
            secs = Some(s);
        } else {
            name.push(words[i].as_str());
        }
        i += 1;
    }
    let name = name.join(" ");
    match (pomodoro, secs) {
        (_, Some(0)) => None,
        (_, Some(s)) if s > LONGEST => None,
        (true, work) => Some(Spec { kind: Kind::Pomodoro, name, secs: work.unwrap_or(POMODORO.0), rest: POMODORO.1 }),
        (false, Some(s)) => Some(Spec { kind: Kind::Countdown, name, secs: s, rest: 0 }),
        (false, None) => Some(Spec { kind: Kind::Stopwatch, name, secs: 0, rest: 0 }),
    }
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

/// `3pm`, `3:30pm`, `7 am`, `15:30` as (hour, minute) and how many words it used.
fn clock_time(words: &[String]) -> Option<((u32, u32), usize)> {
    let first = words.first()?.to_lowercase();
    let (digits, suffix) = first.find(['a', 'p']).map_or((first.as_str(), ""), |at| first.split_at(at));
    let (suffix, used) = match (suffix, words.get(1).map(|w| w.to_lowercase())) {
        ("", Some(next)) if next == "am" || next == "pm" => (next, 2),
        _ => (suffix.to_string(), 1),
    };
    let (h, m) = digits.split_once(':').map_or((digits, "0"), |(h, m)| (h, m));
    let (mut hour, minute): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    match suffix.as_str() {
        "" if hour < 24 => {}
        "am" | "pm" if (1..=12).contains(&hour) => hour = hour % 12 + if suffix == "pm" { 12 } else { 0 },
        _ => return None,
    }
    (minute < 60).then_some(((hour, minute), used))
}

/// Seconds from `now` to the next time the clock shows `(hour, minute)`; a time that is now means tomorrow.
fn until((hour, minute): (u32, u32), now: Clock) -> u64 {
    let target = i64::from(hour * 3600 + minute * 60);
    let at = i64::from(now.hour * 3600 + now.minute * 60 + now.second);
    let diff = (target - at).rem_euclid(86_400);
    if diff == 0 {
        86_400
    } else {
        diff as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: Clock = Clock { hour: 12, minute: 0, second: 0 };

    fn p(line: &str) -> Option<(Kind, String, u64, u64)> {
        parse(line, NOON).map(|s| (s.kind, s.name, s.secs, s.rest))
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
    fn clock_times_count_to_the_next_occurrence() {
        assert_eq!(p("timer until 3pm"), Some((Kind::Countdown, String::new(), 3 * 3600, 0)));
        assert_eq!(p("timer until 3:30 pm"), Some((Kind::Countdown, String::new(), 3 * 3600 + 1800, 0)));
        assert_eq!(p("timer until 11:00").map(|s| s.2), Some(23 * 3600)); // already past today
        assert_eq!(p("timer until 12:00").map(|s| s.2), Some(86_400));
        assert_eq!(p("timer until 13pm"), None);
    }

    #[test]
    fn not_a_timer() {
        assert_eq!(p("timers"), None);
        assert_eq!(p("math"), None);
        assert_eq!(p("timer 0m"), None);
        assert_eq!(p("timer 99h"), None);
    }
}
