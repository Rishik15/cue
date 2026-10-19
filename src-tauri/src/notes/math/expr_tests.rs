//! Purpose: Tests for the line parser: precedence, percentages, names and prose.

use super::expr::{eval, Scope};

fn calc(text: &str) -> Option<f64> {
    eval(text, &Scope::default(), false).map(|(v, _)| (v * 1e6).round() / 1e6)
}

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(calc("12 * 8"), Some(96.0));
    assert_eq!(calc("2 + 3 x 4"), Some(14.0));
    assert_eq!(calc("(2 + 3) × 4"), Some(20.0));
    assert_eq!(calc("2 ^ 3 ^ 2"), Some(512.0));
    assert_eq!(calc("-3 + 5"), Some(2.0));
    assert_eq!(calc("23,405 * 220"), Some(5_149_100.0));
    assert_eq!(calc("5!"), Some(120.0));
    assert_eq!(calc("sqrt 16 + 2"), Some(6.0));
    assert_eq!(calc("round(2.5) + floor(1.9) + ceil(0.1)"), Some(5.0));
    assert_eq!(calc("1 / 0"), None);
}

#[test]
fn percentages() {
    assert_eq!(calc("100 + 15%"), Some(115.0));
    assert_eq!(calc("100 - 15%"), Some(85.0));
    assert_eq!(calc("50% of 200"), Some(100.0));
    assert_eq!(calc("200 * 15%"), Some(30.0));
    assert_eq!(calc("15%"), Some(0.15));
}

#[test]
fn names_ans_and_prose() {
    let mut scope = Scope::default();
    scope.set("trees", 23_405.0);
    scope.set("o2 per tree", 220.0);
    scope.set("o2", 1.0);
    assert_eq!(eval("trees x O2 per tree", &scope, false).map(|(v, _)| v), Some(5_149_100.0));
    scope.ans = Some(10.0);
    assert_eq!(eval("ans * 2", &scope, false).map(|(v, _)| v), Some(20.0));
    assert_eq!(eval("ans * 2", &Scope::default(), false), None);
    assert_eq!(eval("I could never eat that much", &scope, false), None);
    assert_eq!(eval("5 apples x 3", &scope, false), None);
    assert_eq!(eval("5 apples x 3", &scope, true).map(|(v, _)| v), Some(15.0));
    assert_eq!(eval("42", &scope, false), Some((42.0, false)));
    assert_eq!(eval("1 + 1", &scope, false), Some((2.0, true)));
}
