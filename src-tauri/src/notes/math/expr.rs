//! Purpose: Evaluates one line of arithmetic written the way people write it: `+ - * / ^`, brackets, `x` or `×` for times, percentages (`100 + 15%`, `50% of 200`), `n!`, functions (`sqrt`, `ln`, ...), `pi`, `e`, `ans` and the note's named values (names may hold spaces).
//! Contents: Scope — the named values and `ans` a line can use; eval — text to number plus whether it was more than a bare number; the rest is the tokenizer and a precedence parser.
//! A strict line fails on any word it does not know (prose is left alone); a lenient line (one that ends in `=`) skips unknown words, so "5 apples x 3 =" is 15.

use super::units::parse_number;

/// What a line may refer to: named values (lower case, longest name first so "o2 per tree" wins over "o2") and the last result.
#[derive(Default)]
pub struct Scope {
    pub names: Vec<(String, f64)>,
    pub ans: Option<f64>,
}

impl Scope {
    /// Sets or replaces a name.
    pub fn set(&mut self, name: &str, value: f64) {
        let name = name.to_lowercase();
        self.names.retain(|(n, _)| *n != name);
        self.names.push((name, value));
        self.names.sort_by_key(|(n, _)| std::cmp::Reverse(n.chars().count()));
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tok {
    Num(f64),
    Op(char),
    Func(Func),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Func {
    Sqrt,
    Cbrt,
    Ln,
    Log,
    Abs,
    Round,
    Ceil,
    Floor,
}

const FUNCS: &[(&str, Func)] = &[
    ("sqrt", Func::Sqrt),
    ("cbrt", Func::Cbrt),
    ("ln", Func::Ln),
    ("log", Func::Log),
    ("abs", Func::Abs),
    ("round", Func::Round),
    ("ceil", Func::Ceil),
    ("floor", Func::Floor),
];

/// The number a line comes to and whether it was more than a bare number (an operator, a function or a percentage was used), so a line that is just "42" shows no result.
pub fn eval(text: &str, scope: &Scope, lenient: bool) -> Option<(f64, bool)> {
    let tokens = tokenize(text, scope, lenient)?;
    let mut parser = Parser { tokens, at: 0, more: false };
    let value = parser.sum()?;
    (parser.at == parser.tokens.len() && value.v.is_finite()).then_some((value.v, parser.more))
}

fn tokenize(text: &str, scope: &Scope, lenient: bool) -> Option<Vec<Tok>> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() || "$€£¥".contains(c) {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) {
            i = number(&chars, i, &mut out)?;
        } else if c.is_alphabetic() || c == '_' {
            i = word(&chars, i, scope, lenient, &mut out)?;
        } else {
            out.push(Tok::Op(match c {
                '×' | '*' => '*',
                '÷' | '/' => '/',
                '+' | '-' | '^' | '(' | ')' | '!' | '%' => c,
                _ => return None,
            }));
            i += 1;
        }
    }
    Some(out)
}

/// A number with thousands commas ("23,405") or a decimal part.
fn number(chars: &[char], start: usize, out: &mut Vec<Tok>) -> Option<usize> {
    let mut end = start;
    while end < chars.len() {
        let c = chars[end];
        let group = c == ','
            && chars.get(end + 1..end + 4).is_some_and(|g| g.iter().all(char::is_ascii_digit))
            && !chars.get(end + 4).is_some_and(char::is_ascii_digit);
        if !(c.is_ascii_digit() || c == '.' || group) {
            break;
        }
        end += 1;
    }
    let text: String = chars[start..end].iter().collect();
    out.push(Tok::Num(parse_number(&text)?));
    Some(end)
}

/// A name from the note, a function, `of`, `x`, `ans` or a constant; anything else is skipped on a lenient line and fails a strict one.
fn word(chars: &[char], start: usize, scope: &Scope, lenient: bool, out: &mut Vec<Tok>) -> Option<usize> {
    let rest: String = chars[start..].iter().collect::<String>().to_lowercase();
    for (name, value) in &scope.names {
        if rest.starts_with(name.as_str()) && !rest[name.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            out.push(Tok::Num(*value));
            return Some(start + name.chars().count());
        }
    }
    let len = chars[start..].iter().take_while(|c| c.is_alphanumeric() || **c == '_').count();
    let text: String = chars[start..start + len].iter().collect::<String>().to_lowercase();
    let after_value = matches!(out.last(), Some(Tok::Num(_) | Tok::Op(')' | '%' | '!')));
    match text.as_str() {
        "x" | "of" if after_value => out.push(Tok::Op('*')),
        "pi" => out.push(Tok::Num(std::f64::consts::PI)),
        "e" => out.push(Tok::Num(std::f64::consts::E)),
        "ans" => out.push(Tok::Num(scope.ans?)),
        other => match FUNCS.iter().find(|(n, _)| *n == other) {
            Some((_, f)) => out.push(Tok::Func(*f)),
            None if lenient => {}
            None => return None,
        },
    }
    Some(start + len)
}

/// A value and whether it is a percentage (`v` already divided by 100): `100 + 15%` adds 15 % of the left side, anywhere else a percentage is just the fraction.
#[derive(Clone, Copy)]
struct Val {
    v: f64,
    pct: bool,
}

struct Parser {
    tokens: Vec<Tok>,
    at: usize,
    more: bool,
}

impl Parser {
    fn peek(&self) -> Option<Tok> {
        self.tokens.get(self.at).copied()
    }

    fn eat(&mut self, op: char) -> bool {
        let hit = self.peek() == Some(Tok::Op(op));
        self.at += usize::from(hit);
        hit
    }

    fn sum(&mut self) -> Option<Val> {
        let mut left = self.product()?;
        loop {
            let plus = self.eat('+');
            if !plus && !self.eat('-') {
                return Some(left);
            }
            self.more = true;
            let right = self.product()?;
            let amount = if right.pct { left.v * right.v } else { right.v };
            left = Val { v: if plus { left.v + amount } else { left.v - amount }, pct: false };
        }
    }

    fn product(&mut self) -> Option<Val> {
        let mut left = self.power()?;
        loop {
            let times = self.eat('*');
            if !times && !self.eat('/') {
                return Some(left);
            }
            self.more = true;
            let right = self.power()?;
            left = Val { v: if times { left.v * right.v } else { left.v / right.v }, pct: false };
        }
    }

    fn power(&mut self) -> Option<Val> {
        let base = self.unary()?;
        if !self.eat('^') {
            return Some(base);
        }
        self.more = true;
        let exponent = self.power()?; // right to left: 2 ^ 3 ^ 2 is 2 ^ 9
        Some(Val { v: base.v.powf(exponent.v), pct: false })
    }

    fn unary(&mut self) -> Option<Val> {
        if self.eat('-') {
            self.more = true;
            let inner = self.unary()?;
            return Some(Val { v: -inner.v, pct: inner.pct });
        }
        self.eat('+');
        let mut value = self.atom()?;
        loop {
            if self.eat('!') {
                self.more = true;
                value = Val { v: factorial(value.v)?, pct: false };
            } else if self.eat('%') {
                self.more = true;
                value = Val { v: value.v / 100.0, pct: true };
            } else {
                return Some(value);
            }
        }
    }

    fn atom(&mut self) -> Option<Val> {
        let tok = self.peek()?;
        self.at += 1;
        match tok {
            Tok::Num(v) => Some(Val { v, pct: false }),
            Tok::Op('(') => {
                let inner = self.sum()?;
                self.eat(')').then_some(inner)
            }
            Tok::Func(f) => {
                self.more = true;
                let arg = self.unary()?;
                Some(Val { v: apply(f, arg.v)?, pct: false })
            }
            Tok::Op(_) => None,
        }
    }
}

fn apply(f: Func, v: f64) -> Option<f64> {
    let out = match f {
        Func::Sqrt => v.sqrt(),
        Func::Cbrt => v.cbrt(),
        Func::Ln => v.ln(),
        Func::Log => v.log10(),
        Func::Abs => v.abs(),
        Func::Round => v.round(),
        Func::Ceil => v.ceil(),
        Func::Floor => v.floor(),
    };
    out.is_finite().then_some(out)
}

/// n! for whole numbers up to 170 (the largest that fits).
fn factorial(n: f64) -> Option<f64> {
    (n >= 0.0 && n.fract() == 0.0 && n <= 170.0).then(|| (2..=n as u32).map(f64::from).product())
}
