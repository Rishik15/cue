//! File: notes/math/mod.rs
//! Purpose: Calculations inside a note, on any line: arithmetic and percentages, named values (`guests : 9`), unit conversions, `ans` for the last result, and `sum =`, `avg =`, `count =` over the lines above. Offline and pure.
//! Contents: evaluate — a note's text to one `Line` per line (its result, and whether it is a calculation still waiting for its `=`); show — a number as text; work, calculation, aggregate, definition — how one line is read. `expr` is the parser, `units` the conversions, `format` the number display.
//! A line shows a result when it ends in `=` or is a unit conversion; prose and `//` comment lines show nothing. A result is a whole number when it is one and no number in the line had a decimal point (`5 + 9 = 14`), else it has the chosen decimals (`10 / 4 = 2.50`, `2.5 + 2.5 = 5.00`).

mod expr;
mod format;
mod units;

use expr::{eval, Scope, MAX_NAME};

/// How results are written.
#[derive(Clone, Copy)]
pub struct Look {
    pub digits: usize,
    pub thousands: bool,
}

/// Text of a result for a number, as the note shows it. Inputs: the number, how to write it. Returns the text.
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

/// The result as text: a whole number stays whole unless the line was written with decimals. Inputs: what the line came to, how to write it. Returns the text, with its unit if any.
fn text(worked: &Worked, look: &Look) -> String {
    let whole = !worked.decimal && worked.value.fract() == 0.0 && worked.value.abs() < 1e15;
    let number = show(worked.value, &Look { digits: if whole { 0 } else { look.digits }, ..*look });
    match worked.unit {
        Some(unit) => format!("{number} {unit}"),
        None => number,
    }
}

/// Input: the note's text and how to write results. Returns one `Line` per line of the note. Every line of a note is read the same way, so math works beside lists, prose and timers. A result shows after a `=` that ends the line (`12 x 8 =`) and after a plain conversion (`10 ft to m`); `sum =`, `avg =` and `count =` work out the numbers of the lines above, back to the last blank line.
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
                scope.ans = Some((worked.value, worked.decimal));
            }
            block_add(&mut block, line, &worked);
            let value = || text(&worked, look);
            Line { shown: worked.shown.then(value), ready: worked.ready, value: worked.defined.then(value) }
        })
        .collect()
}

/// A line's value joins the block it is in, except a total (that would count twice in the next one), also on a list line. Inputs: the block, the line, what it came to. Returns nothing.
fn block_add(block: &mut Vec<(f64, bool)>, line: &str, worked: &Worked) {
    if aggregate(without_marker(line.trim()).trim_end_matches('=').trim()).is_none() {
        block.push((worked.value, worked.decimal));
    }
}

/// Works out one line. Inputs: the line, the names so far (a name it defines is added), the numbers of the block above. Returns what it came to, or None for prose, a comment, a line too long or one that is not math.
fn work(line: &str, scope: &mut Scope, block: &[(f64, bool)]) -> Option<Worked> {
    let line = without_marker(line.trim());
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

/// The line without its list marker (`- `, `* `, `3. `, `[ ] `, `[x] `, alone or together), so lists can hold calculations. Input: the trimmed line. Returns the rest.
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

/// What `body` comes to. Before a `=` a calculation may follow words (`milk 2 x 3 =`, `- [ ] 4 + 4 =`): the longest tail that parses and uses an operator counts. Inputs: the text, the scope, whether it ended in `=`. Returns the value, whether a number had a decimal point and whether it was more than a bare number.
fn calculation(body: &str, scope: &Scope, equals: bool) -> Option<(f64, bool, bool)> {
    if let Some(found) = eval(body, scope, equals) {
        return Some(found);
    }
    if !equals {
        return None;
    }
    let starts = body.char_indices().filter(|(i, c)| *i > 0 && c.is_whitespace()).map(|(i, c)| i + c.len_utf8());
    starts.take(40).find_map(|at| eval(&body[at..], scope, true).filter(|(.., more)| *more))
}

/// A total over the numbers above: None for an empty block. shortcut: each total rescans its block, so a block of n numbers with n totals costs n squared; keep running sums per block if notes of that shape appear.
type Total = fn(&[(f64, bool)]) -> Option<(f64, bool)>;

/// `sum`, `avg` or `count` as a total. Input: the word. Returns the total, or None for any other word.
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

/// `name : expression` splits into the name and the expression; anything else is all expression. Names are short plain words (they may hold spaces) and do not start with a digit, so `10:30` is not one. Input: the line. Returns the name, if any, and the expression.
fn definition(line: &str) -> (Option<&str>, &str) {
    match line.split_once(':') {
        Some((name, body)) if valid_name(name.trim()) => (Some(name.trim()), body),
        _ => (None, line),
    }
}

/// Whether `name` can name a value: plain letters, digits, spaces, `_`, `.` or an apostrophe, starting with a letter, at most `MAX_NAME` characters. Returns the answer.
fn valid_name(name: &str) -> bool {
    let plain = name.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '.' | '\'' | '’'));
    plain && name.chars().next().is_some_and(char::is_alphabetic) && name.chars().count() <= MAX_NAME
}
