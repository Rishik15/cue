//! Purpose: Clock times for `timer until ...`: reads `3pm`, `3:30 pm`, `15:30` and counts the seconds to their next occurrence.
//! Contents: Clock — the time of day to count from; clock_time — words to (hour, minute) and how many words were used; until — seconds to the next occurrence.

/// The time of day, for `until`.
#[derive(Clone, Copy)]
pub struct Clock {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// `3pm`, `3:30pm`, `7 am`, `15:30` as (hour, minute) and how many words it used.
pub fn clock_time(words: &[String]) -> Option<((u32, u32), usize)> {
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
pub fn until((hour, minute): (u32, u32), now: Clock) -> u64 {
    let target = i64::from(hour * 3600 + minute * 60);
    let at = i64::from(now.hour * 3600 + now.minute * 60 + now.second);
    let diff = (target - at).rem_euclid(86_400);
    if diff == 0 {
        86_400
    } else {
        diff as u64
    }
}
