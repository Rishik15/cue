//! Purpose: Lays a local time out by layout tokens and moves it by an offset (`+3d`, `-1w`). Pure: the time comes in as a value.
//! Contents: format_time — layout tokens to text; parse_offset — `+3d` to (3, 'd'); shift — a time moved by minutes, hours, days, weeks, months or years; the civil-date helpers behind it.

use crate::platform::LocalTime;

const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
/// Offsets beyond this are refused, so arithmetic cannot overflow.
const MAX_OFFSET: i64 = 100_000;

/// Lays a time out by tokens: YYYY YY MMMM MMM MM M DD D dddd ddd HH H hh h mm m ss s A a. Text in square brackets is copied as it is.
pub fn format_time(layout: &str, t: &LocalTime) -> String {
    let (chars, mut out, mut i) = (layout.chars().collect::<Vec<_>>(), String::new(), 0);
    while i < chars.len() {
        if chars[i] == '[' {
            let close = chars[i..].iter().position(|&c| c == ']').map_or(chars.len(), |p| i + p);
            out.extend(&chars[i + 1..close.min(chars.len())]);
            i = close + 1;
            continue;
        }
        let run = chars[i..].iter().take_while(|&&c| c == chars[i]).count();
        match token(chars[i], run, t) {
            Some(text) => out.push_str(&text),
            None => out.extend(&chars[i..i + run]),
        }
        i += run;
    }
    out
}

fn token(c: char, run: usize, t: &LocalTime) -> Option<String> {
    let name = |list: &[&str], index: u32| list.get(index as usize).copied().unwrap_or("").to_string();
    let hour12 = if t.hour.is_multiple_of(12) { 12 } else { t.hour % 12 };
    let two = |n: u32| format!("{n:02}");
    Some(match (c, run) {
        ('Y', 4) => t.year.to_string(),
        ('Y', 2) => format!("{:02}", t.year.rem_euclid(100)),
        ('M', 4) => name(&MONTHS, t.month.saturating_sub(1)),
        ('M', 3) => name(&MONTHS, t.month.saturating_sub(1)).chars().take(3).collect(),
        ('M', 2) => two(t.month),
        ('M', 1) => t.month.to_string(),
        ('D', 2) => two(t.day),
        ('D', 1) => t.day.to_string(),
        ('d', 4) => name(&DAYS, t.weekday),
        ('d', 3) => name(&DAYS, t.weekday).chars().take(3).collect(),
        ('H', 2) => two(t.hour),
        ('H', 1) => t.hour.to_string(),
        ('h', 2) => two(hour12),
        ('h', 1) => hour12.to_string(),
        ('m', 2) => two(t.minute),
        ('m', 1) => t.minute.to_string(),
        ('s', 2) => two(t.second),
        ('s', 1) => t.second.to_string(),
        ('A', 1) => (if t.hour < 12 { "AM" } else { "PM" }).into(),
        ('a', 1) => (if t.hour < 12 { "am" } else { "pm" }).into(),
        _ => return None,
    })
}

/// `+3d`, `-2w`, `+1M`: a sign, a number and a unit (m minutes, h hours, d days, w weeks, M months, y years).
pub fn parse_offset(text: &str) -> Option<(i64, char)> {
    let sign = match text.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let rest = &text[1..];
    let unit = rest.chars().last().filter(|u| "mhdwMy".contains(*u))?;
    let number: i64 = rest[..rest.len() - 1].parse().ok().filter(|n| *n <= MAX_OFFSET)?;
    Some((sign * number, unit))
}

pub fn shift(t: &LocalTime, amount: i64, unit: char) -> LocalTime {
    if matches!(unit, 'M' | 'y') {
        let months = if unit == 'y' { amount * 12 } else { amount };
        let index = i64::from(t.year) * 12 + i64::from(t.month) - 1 + months;
        let (year, month) = (index.div_euclid(12), index.rem_euclid(12) + 1);
        let day = i64::from(t.day).min(days_from_civil(year, month + 1, 1) - days_from_civil(year, month, 1));
        let days = days_from_civil(year, month, day);
        return LocalTime { year: year as i32, month: month as u32, day: day as u32, weekday: weekday(days), ..*t };
    }
    let step = match unit {
        'm' => 60,
        'h' => 3600,
        'd' => 86_400,
        _ => 604_800,
    };
    let clock = i64::from(t.hour * 3600 + t.minute * 60 + t.second);
    let seconds = days_from_civil(i64::from(t.year), i64::from(t.month), i64::from(t.day)) * 86_400 + clock + amount * step;
    let (days, of_day) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    LocalTime {
        year: year as i32,
        month: month as u32,
        day: day as u32,
        hour: (of_day / 3600) as u32,
        minute: (of_day % 3600 / 60) as u32,
        second: (of_day % 60) as u32,
        weekday: weekday(days),
    }
}

/// 1970-01-01 was a Thursday.
fn weekday(days: i64) -> u32 {
    (days + 4).rem_euclid(7) as u32
}

/// Days since 1970-01-01 of a civil date; a month of 13 means January of the next year.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let (year, month) = if month > 12 { (year + 1, month - 12) } else { (year, month) };
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, doy - (153 * mp + 2) / 5 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(year: i32, month: u32, day: u32, hour: u32) -> LocalTime {
        LocalTime { year, month, day, hour, minute: 30, second: 0, weekday: 1 }
    }

    #[test]
    fn offsets_parse_with_a_sign_a_number_and_a_unit() {
        assert_eq!(parse_offset("+3d"), Some((3, 'd')));
        assert_eq!(parse_offset("-2M"), Some((-2, 'M')));
        assert_eq!([parse_offset("3d"), parse_offset("+d"), parse_offset("+3x"), parse_offset("+999999d")], [None; 4]);
    }

    #[test]
    fn days_roll_over_months_years_and_leap_days() {
        let t = shift(&at(2026, 12, 30, 23), 3, 'd');
        assert_eq!((t.year, t.month, t.day, t.weekday), (2027, 1, 2, 6));
        assert_eq!((shift(&at(2024, 2, 28, 0), 2, 'd').month, shift(&at(2024, 2, 28, 0), 2, 'd').day), (3, 1));
        let t = shift(&at(2026, 6, 1, 0), -1, 'd');
        assert_eq!((t.month, t.day, t.weekday), (5, 31, 0));
    }

    #[test]
    fn hours_and_weeks_move_the_clock_and_the_date() {
        let t = shift(&at(2026, 6, 1, 23), 2, 'h');
        assert_eq!((t.day, t.hour, t.minute), (2, 1, 30));
        assert_eq!(shift(&at(2026, 6, 1, 0), 1, 'w').day, 8);
    }

    #[test]
    fn months_and_years_clamp_the_day_to_the_month() {
        let t = shift(&at(2026, 1, 31, 0), 1, 'M');
        assert_eq!((t.month, t.day), (2, 28));
        let t = shift(&at(2024, 2, 29, 0), 1, 'y');
        assert_eq!((t.year, t.month, t.day), (2025, 2, 28));
        assert_eq!(shift(&at(2026, 1, 15, 0), -2, 'M').year, 2025);
    }
}
