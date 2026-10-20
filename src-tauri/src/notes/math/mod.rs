//! Purpose: Calculations inside a note, on any line: arithmetic and percentages, named values (`guests : 9`), unit conversions, `ans` for the last result, and `sum =`, `avg =`, `count =` over the lines above. Offline and pure.
//! Contents: evaluate — a note's text to one `Line` per line (its result, and whether it is a calculation still waiting for its `=`). `expr` is the parser, `units` the conversions, `format` the number display.
//! A line shows a result when it ends in `=` or is a unit conversion; prose and `//` comment lines show nothing. A result is a whole number when it is one and no number in the line had a decimal point (`5 + 9 = 14`), else it has the chosen decimals (`10 / 4 = 2.50`, `2.5 + 2.5 = 5.00`).

mod expr;
#[cfg(test)]
mod expr_tests;
mod format;
mod units;

use expr::{eval, Scope};

/// How results are written.
#[derive(Clone, Copy)]
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

/// What the page needs to know about one line: the result it shows, and whether it is a calculation without its `=` yet (Enter at its end adds it).
#[derive(Default)]
pub struct Line {
    pub shown: Option<String>,
    pub ready: bool,
    /// The value the line gave to the name it defines (`my friends: 52` is "52"), for the page's name list.
    pub value: Option<String>,
}

/// What a line came to. `decimal`: a number in it had a decimal point.
struct Worked {
    value: f64,
    unit: Option<&'static str>,
    shown: bool,
    decimal: bool,
    ready: bool,
    /// The line defined a name.
    defined: bool,
}

/// The result as text: a whole number stays whole unless the line was written with decimals.
fn text(worked: &Worked, look: &Look) -> String {
    let whole = !worked.decimal && worked.value.fract() == 0.0 && worked.value.abs() < 1e15;
    let number = show(worked.value, &Look { digits: if whole { 0 } else { look.digits }, ..*look });
    match worked.unit {
        Some(unit) => format!("{number} {unit}"),
        None => number,
    }
}

/// Every line of a note is read the same way, so math works beside lists, prose and timers. A result shows after a `=` that ends the line (`12 x 8 =`) and after a plain conversion (`10 ft to m`); `sum =`, `avg =` and `count =` work out the numbers of the lines above, back to the last blank line.
pub fn evaluate(text_of_note: &str, look: &Look) -> Vec<Line> {
    let mut scope = Scope::default();
    let mut block: Vec<(f64, bool)> = Vec::new();
    text_of_note
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                block.clear();
                return Line::default();
            }
            let Some(worked) = work(line, &mut scope, &block) else { return Line::default() };
            if worked.shown {
                scope.ans = Some((worked.value, worked.decimal)); // `ans` is the last result on screen, not a number that was only stored
            }
            block_add(&mut block, line, &worked);
            let value = || text(&worked, look);
            Line { shown: worked.shown.then(value), ready: worked.ready, value: worked.defined.then(value) }
        })
        .collect()
}

/// A line's value joins the block it is in, except a total (that would count twice in the next one).
fn block_add(block: &mut Vec<(f64, bool)>, line: &str, worked: &Worked) {
    if aggregate(line.trim().trim_end_matches('=').trim()).is_none() {
        block.push((worked.value, worked.decimal));
    }
}

fn work(line: &str, scope: &mut Scope, block: &[(f64, bool)]) -> Option<Worked> {
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
        if let Some((value, decimal)) = aggregate(body).and_then(|f| f(block)) {
            return Some(Worked { value, unit: None, shown: true, decimal, ready: false, defined: false });
        }
    }
    let (value, unit, decimal, more) = match units::convert(body) {
        Some((value, unit, decimal)) => (value, Some(unit), decimal, true),
        None => {
            let (value, decimal, more) = calculation(body, scope, equals)?;
            (value, None, decimal, more)
        }
    };
    if let Some(name) = name {
        scope.set(name, value, decimal);
    }
    Some(Worked {
        shown: equals || unit.is_some(),
        ready: !equals && unit.is_none() && more,
        defined: name.is_some(),
        value,
        unit,
        decimal,
    })
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
fn calculation(body: &str, scope: &Scope, equals: bool) -> Option<(f64, bool, bool)> {
    if let Some(found) = eval(body, scope, equals) {
        return Some(found);
    }
    if !equals {
        return None;
    }
    let starts = body.char_indices().filter(|(i, c)| *i > 0 && c.is_whitespace()).map(|(i, _)| i + 1);
    starts.take(40).find_map(|at| eval(&body[at..], scope, true).filter(|(.., more)| *more))
}

/// A total over the numbers above: None for an empty block.
type Total = fn(&[(f64, bool)]) -> Option<(f64, bool)>;

/// `sum`, `avg` or `count` as a total.
fn aggregate(word: &str) -> Option<Total> {
    match word.to_lowercase().as_str() {
        "sum" | "total" => Some(|v| (!v.is_empty()).then(|| (v.iter().map(|n| n.0).sum(), v.iter().any(|n| n.1)))),
        "avg" | "average" => {
            Some(|v| (!v.is_empty()).then(|| (v.iter().map(|n| n.0).sum::<f64>() / v.len() as f64, v.iter().any(|n| n.1))))
        }
        "count" => Some(|v| (!v.is_empty()).then_some((v.len() as f64, false))),
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
    let plain = name.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '.' | '\'' | '’'));
    plain && name.chars().next().is_some_and(char::is_alphabetic) && name.chars().count() <= 40
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOK: Look = Look { digits: 2, thousands: true };

    fn run(text: &str) -> Vec<Option<String>> {
        evaluate(text, &LOOK).into_iter().map(|l| l.shown).collect()
    }

    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn math_works_anywhere_in_a_note() {
        let got = run("Shopping\n- [ ] milk 2 x 3 =\nhello\n12 * 8 =");
        assert_eq!(got, vec![None, some("6"), None, some("96")]);
    }

    #[test]
    fn names_chain_and_results_show_when_asked() {
        let note =
            "trees: 23,405\nO2 per tree: 220\n\ntotal O2: trees x O2 per tree =\nO2 per species: total O2 / 1205 =\n2 + 3\n42\nans * 2 =";
        let got = run(note);
        assert_eq!(got[0], None); // a plain definition shows nothing
        assert_eq!(got[3], some("5,149,100"));
        assert_eq!(got[4], some("4,273.11"));
        assert_eq!(got[5], None); // no `=`, no answer
        assert_eq!(got[6], None); // a bare number
        assert_eq!(got[7], some("8,546.22")); // ans is the last result shown
    }

    #[test]
    fn list_markers_do_not_hide_a_calculation() {
        let got = run("- price: 12\n* [x] tax: 8\n1. sum =\n- 10 ft to m");
        assert_eq!(got[2], some("20"));
        assert_eq!(got[3], some("3.05 m"));
    }

    #[test]
    fn sum_avg_and_count_read_the_block_above() {
        let got = run("a: 10\n20\n$30\nsum =\navg =\ncount =\n\nsum =");
        assert_eq!(&got[3..6], &[some("60"), some("20"), some("3")]);
        assert_eq!(got[7], None); // a new block starts at a blank line
    }

    #[test]
    fn whole_numbers_stay_whole_and_decimals_keep_their_places() {
        let got = run("5+9 =\n10 / 4 =\n2.5 + 2.5 =\n10 / 5 =\n1000 g to kg\n$5 + 9.50 =");
        assert_eq!(got, vec![some("14"), some("2.50"), some("5.00"), some("2"), some("1 kg"), some("14.50")]);
    }

    #[test]
    fn names_hand_their_values_to_the_page_and_may_hold_apostrophes() {
        let lines = evaluate("my friends: 52\nrob’s friends: 25\nattendees: my friends + rob’s friends =\nfood: $2.30", &LOOK);
        let values: Vec<Option<String>> = lines.iter().map(|l| l.value.clone()).collect();
        assert_eq!(values, vec![some("52"), some("25"), some("77"), some("2.30")]);
        assert_eq!(lines[2].shown, some("77"));
    }

    #[test]
    fn a_calculation_without_its_equals_is_ready() {
        let lines = evaluate("5+9\n42\n5+9 =\nhello there\n10 ft to m\nx: 2 * 3", &LOOK);
        let ready: Vec<bool> = lines.iter().map(|l| l.ready).collect();
        assert_eq!(ready, vec![true, false, false, false, false, true]); // a bare number, a finished line, prose and a conversion (which already shows) are not
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
