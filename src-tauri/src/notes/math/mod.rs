//! Purpose: Calculations inside a note. A note whose first line is `math` is worked out line by line: arithmetic and percentages, named values (`guests : 9`), unit conversions, `ans` for the last result. Offline and pure.
//! Contents: evaluate — a note's text to one optional result per line; is_math — whether a first line switches this on. `expr` is the parser, `units` the conversions, `format` the number display.
//! A line shows a result when it ends in `=` or is plainly a calculation; prose and `//` comment lines show nothing.

mod expr;
#[cfg(test)]
mod expr_tests;
mod format;
mod units;

use expr::{eval, Scope};

/// How results are written.
pub struct Look {
    pub digits: usize,
    pub thousands: bool,
}

/// `math`, or `math: A title`.
pub fn is_math(first_line: &str) -> bool {
    let first = first_line.trim().to_lowercase();
    first == "math" || first.starts_with("math:")
}

/// Text of a result for a number, as the note shows it.
pub fn show(value: f64, look: &Look) -> String {
    format::show(value, look.digits, look.thousands)
}

pub fn evaluate(text: &str, look: &Look) -> Vec<Option<String>> {
    let lines: Vec<&str> = text.split('\n').collect();
    if !is_math(lines[0]) {
        return vec![None; lines.len()];
    }
    let mut scope = Scope::default();
    let rest = lines[1..].iter().map(|line| line_result(line, &mut scope, look));
    std::iter::once(None).chain(rest).collect()
}

/// The longest line worked out, in bytes.
const MAX_LINE: usize = 300;

fn line_result(line: &str, scope: &mut Scope, look: &Look) -> Option<String> {
    let line = line.trim();
    // A line this long is prose, and a very long one would nest the parser deeper than its stack allows.
    if line.is_empty() || line.starts_with("//") || line.len() > MAX_LINE {
        return None;
    }
    let (name, body) = definition(line);
    let body = body.trim_end();
    let equals = body.ends_with('=');
    let body = body.strip_suffix('=').unwrap_or(body).trim();
    let (value, unit, more) = match units::convert(body) {
        Some((value, unit)) => (value, Some(unit), true),
        None => {
            let (value, more) = eval(body, scope, equals)?;
            (value, None, more)
        }
    };
    if let Some(name) = name {
        scope.set(name, value);
    }
    let shown = equals || more;
    if shown {
        scope.ans = Some(value); // `ans` is the last result on screen, not a number that was only stored
    }
    shown.then(|| match unit {
        Some(unit) => format!("{} {unit}", show(value, look)),
        None => show(value, look),
    })
}

/// `name : expression` splits into the name and the expression; anything else is all expression. Names are short plain words (they may hold spaces) and do not start with a digit, so `10:30` is not one.
fn definition(line: &str) -> (Option<&str>, &str) {
    match line.split_once(':') {
        Some((name, body)) if valid_name(name.trim()) => (Some(name.trim()), body),
        _ => (None, line),
    }
}

fn valid_name(name: &str) -> bool {
    let plain = name.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '_' || c == '.');
    plain && name.chars().next().is_some_and(char::is_alphabetic) && name.chars().count() <= 40
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOK: Look = Look { digits: 2, thousands: true };

    fn run(text: &str) -> Vec<Option<String>> {
        evaluate(text, &LOOK)
    }

    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn a_note_without_the_keyword_shows_nothing() {
        assert_eq!(run("12 * 8 ="), vec![None]);
        assert_eq!(run("hello\n1 + 1"), vec![None, None]);
    }

    #[test]
    fn names_chain_and_results_show_when_asked_or_obvious() {
        let note = "math\ntrees: 23,405\nO2 per tree: 220\n\ntotal O2: trees x O2 per tree =\nO2 per species: total O2 / 1205 =\n2 + 3\n42\nans * 2";
        let got = run(note);
        assert_eq!(got[1], None); // a plain definition shows nothing
        assert_eq!(got[4], some("5,149,100.00"));
        assert_eq!(got[5], some("4,273.11"));
        assert_eq!(got[6], some("5.00"));
        assert_eq!(got[7], None); // a bare number
        assert_eq!(got[8], some("10.00"));
    }

    #[test]
    fn units_comments_and_prose() {
        let got = run("math: Trip\n// 5 + 5\n8900 miles to km\n10 ft to m =\nI could never eat that much.\nNote: remember this\n10:30 + 5");
        assert_eq!(got[1], None);
        assert_eq!(got[2], some("14,323.16 km"));
        assert_eq!(got[3], some("3.05 m"));
        assert_eq!(got[4], None);
        assert_eq!(got[5], None);
        assert_eq!(got[6], None);
    }
}
