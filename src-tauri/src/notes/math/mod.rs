//! Purpose: Calculations inside a note, on any line: arithmetic and percentages, named values (`guests : 9`), unit conversions, `ans` for the last result, and `sum =`, `avg =`, `count =` over the lines above. Offline and pure.
//! Contents: evaluate — a note's text to one optional result per line. `expr` is the parser, `units` the conversions, `format` the number display.
//! A line shows a result when it ends in `=` or is a unit conversion; prose and `//` comment lines show nothing.

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

/// Text of a result for a number, as the note shows it.
pub fn show(value: f64, look: &Look) -> String {
    format::show(value, look.digits, look.thousands)
}

/// The longest line worked out, in bytes.
const MAX_LINE: usize = 300;

/// What a line came to.
struct Worked {
    value: f64,
    unit: Option<&'static str>,
    shown: bool,
}

/// Every line of a note is read the same way, so math works beside lists, prose and timers. A result shows after a `=` that ends the line (`12 x 8 =`) and after a plain conversion (`10 ft to m`); `sum =`, `avg =` and `count =` work out the numbers of the lines above, back to the last blank line.
pub fn evaluate(text: &str, look: &Look) -> Vec<Option<String>> {
    let mut scope = Scope::default();
    let mut block: Vec<f64> = Vec::new();
    text.split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                block.clear();
                return None;
            }
            let worked = work(line, &mut scope, &block)?;
            if worked.shown {
                scope.ans = Some(worked.value); // `ans` is the last result on screen, not a number that was only stored
            }
            let shown = worked.shown.then(|| match &worked.unit {
                Some(unit) => format!("{} {unit}", show(worked.value, look)),
                None => show(worked.value, look),
            });
            block_add(&mut block, line, &worked);
            shown
        })
        .collect()
}

/// A line's value joins the block it is in, except a total (that would count twice in the next one).
fn block_add(block: &mut Vec<f64>, line: &str, worked: &Worked) {
    if aggregate(line.trim().trim_end_matches('=').trim()).is_none() {
        block.push(worked.value);
    }
}

fn work(line: &str, scope: &mut Scope, block: &[f64]) -> Option<Worked> {
    let line = without_marker(line.trim());
    // A line this long is prose, and a very long one would nest the parser deeper than its stack allows.
    if line.starts_with("//") || line.len() > MAX_LINE {
        return None;
    }
    let (name, body) = definition(line);
    let body = body.trim_end();
    let equals = body.ends_with('=');
    let body = body.strip_suffix('=').unwrap_or(body).trim();
    if equals {
        if let Some(total) = aggregate(body).and_then(|f| f(block)) {
            return Some(Worked { value: total, unit: None, shown: true });
        }
    }
    let (value, unit) = match units::convert(body) {
        Some((value, unit)) => (value, Some(unit)),
        None => (calculation(body, scope, equals)?, None),
    };
    if let Some(name) = name {
        scope.set(name, value);
    }
    Some(Worked { shown: equals || unit.is_some(), value, unit })
}

/// The line without its list marker (`- `, `* `, `3. `, `[ ] `, `[x] `, alone or together), so lists can hold calculations.
fn without_marker(mut line: &str) -> &str {
    for _ in 0..2 {
        let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
        line = match (line.strip_prefix(['-', '*']), digits.strip_prefix(['.', ')'])) {
            (Some(rest), _) | (None, Some(rest)) if rest.starts_with(' ') => rest.trim_start(),
            _ => line,
        };
        for open in ["[ ]", "[x]", "[X]", "[]"] {
            line = line.strip_prefix(open).map_or(line, str::trim_start);
        }
    }
    line
}

/// What `body` comes to. Before a `=` a calculation may follow words (`milk 2 x 3 =`, `- [ ] 4 + 4 =`): the longest tail that parses and uses an operator counts.
fn calculation(body: &str, scope: &Scope, equals: bool) -> Option<f64> {
    if let Some((value, _)) = eval(body, scope, equals) {
        return Some(value);
    }
    if !equals {
        return None;
    }
    let starts = body.char_indices().filter(|(i, c)| *i > 0 && c.is_whitespace()).map(|(i, _)| i + 1);
    starts.take(40).find_map(|at| eval(&body[at..], scope, true).filter(|(_, more)| *more).map(|(value, _)| value))
}

/// A total over the numbers above: None for an empty block.
type Total = fn(&[f64]) -> Option<f64>;

/// `sum`, `avg` or `count` as a total.
fn aggregate(word: &str) -> Option<Total> {
    match word.to_lowercase().as_str() {
        "sum" | "total" => Some(|v| (!v.is_empty()).then(|| v.iter().sum())),
        "avg" | "average" => Some(|v| (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)),
        "count" => Some(|v| (!v.is_empty()).then_some(v.len() as f64)),
        _ => None,
    }
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
    fn math_works_anywhere_in_a_note() {
        let got = run("Shopping\n- [ ] milk 2 x 3 =\nhello\n12 * 8 =");
        assert_eq!(got, vec![None, some("6.00"), None, some("96.00")]);
    }

    #[test]
    fn names_chain_and_results_show_when_asked() {
        let note =
            "trees: 23,405\nO2 per tree: 220\n\ntotal O2: trees x O2 per tree =\nO2 per species: total O2 / 1205 =\n2 + 3\n42\nans * 2 =";
        let got = run(note);
        assert_eq!(got[0], None); // a plain definition shows nothing
        assert_eq!(got[3], some("5,149,100.00"));
        assert_eq!(got[4], some("4,273.11"));
        assert_eq!(got[5], None); // no `=`, no answer
        assert_eq!(got[6], None); // a bare number
        assert_eq!(got[7], some("8,546.22")); // ans is the last result shown
    }

    #[test]
    fn list_markers_do_not_hide_a_calculation() {
        let got = run("- price: 12\n* [x] tax: 8\n1. sum =\n- 10 ft to m");
        assert_eq!(got[2], some("20.00"));
        assert_eq!(got[3], some("3.05 m"));
    }

    #[test]
    fn sum_avg_and_count_read_the_block_above() {
        let got = run("a: 10\n20\n$30\nsum =\navg =\ncount =\n\nsum =");
        assert_eq!(&got[3..6], &[some("60.00"), some("20.00"), some("3.00")]);
        assert_eq!(got[7], None); // a new block starts at a blank line
    }

    #[test]
    fn units_comments_and_prose() {
        let got = run("Trip\n// 5 + 5 =\n8900 miles to km\n10 ft to m =\nI could never eat that much.\nNote: remember this\n10:30 + 5");
        assert_eq!(got[1], None);
        assert_eq!(got[2], some("14,323.16 km"));
        assert_eq!(got[3], some("3.05 m"));
        assert_eq!(got[4], None);
        assert_eq!(got[5], None);
        assert_eq!(got[6], None);
    }
}
