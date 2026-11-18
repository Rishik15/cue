//! File: notes/tests/math.rs
//! Purpose: Protect the calculations in a note: the README example works line by line, totals cover the lines above, prose is never touched and bad input shows nothing instead of nonsense.
//! Contents: scenario tests of `eval::answers`, the result the note window paints.

use crate::notes::eval::{answers, Evaluated};

/// The results of a note with two decimals and no thousands separator. Input: the note. Returns what the page gets.
fn worked(note: &str) -> Evaluated {
    answers(note, 2, false)
}

/// Shorthand for the expected result column. Input: the shown results. Returns them as the page receives them.
fn column(shown: &[&str]) -> Vec<Option<String>> {
    shown.iter().map(|s| (!s.is_empty()).then(|| s.to_string())).collect()
}

/// The note from the README: arithmetic, a named value, a conversion, a checklist line, a lenient line with words, `ans`, and the whole-number versus decimals rule.
#[test]
fn the_readme_note_works_line_by_line() {
    let note = "12 x 8 =\nrent: 900\nrent / 4 =\n10 ft to m\n[ ] book the flights\n\n5 apples x 3 =\n6 x 7 =\nans + 8 =\n10 / 4 =\n5 + 9 =\n2.5 + 2.5 =";
    let out = worked(note);
    assert_eq!(out.results, column(&["96", "", "225", "3.05 m", "", "", "15", "42", "50", "2.50", "14", "5.00"]));
    assert_eq!(out.values[1].as_deref(), Some("900"));
}

/// `sum`, `avg` and `count` cover the numbers above back to the last blank line, and a total is not counted again by the next one.
#[test]
fn totals_cover_the_lines_above_up_to_a_blank_line() {
    let out = worked("2 x 3 =\n4.5 x 2 =\nsum =\navg =\ncount =\n\nsum =");
    assert_eq!(out.results, column(&["6", "9.00", "15.00", "7.50", "2", "", ""]));
}

/// Percentages work the way they are written, and the display follows the chosen decimals and thousands separator.
#[test]
fn percentages_decimals_and_thousands_follow_the_settings() {
    assert_eq!(worked("100 + 15% =\n50% of 200 =").results, column(&["115", "100"]));
    let big = answers("1200 x 1200 =\n1 / 3 =", 3, true);
    assert_eq!(big.results, column(&["1,440,000", "0.333"]));
}

/// Conversions answer in the target unit; units of different kinds, or unknown ones, show nothing.
#[test]
fn unit_conversions_answer_in_the_target_unit() {
    let out = worked("72 f to c\n5 km to mi\n1,000 m to km\n10 kg to ft\n5 widgets to parsecs");
    assert_eq!(out.results, column(&["22.22 c", "3.11 mi", "1 km", "", ""]));
}

/// Names can hold spaces and be used on list lines; a list marker never blocks a calculation.
#[test]
fn named_values_and_list_markers_work_together() {
    let out = worked("guests : 9\nguests x 2 =\n- [ ] 4 + 4 =\n3. guests + 1 =");
    assert_eq!(out.results, column(&["", "18", "8", "10"]));
    assert_eq!(out.values[0].as_deref(), Some("9"));
}

/// Prose, comments, a bare number and a time show nothing; a calculation that only lacks its `=` is marked ready; division by zero shows nothing.
#[test]
fn prose_and_bad_input_show_nothing_and_unfinished_sums_are_marked() {
    let out = worked("Dinner with Sam at 7\n// 2 + 2 =\n42\n3 + 4\n10:30\n1 / 0 =");
    assert_eq!(out.results, column(&["", "", "", "", "", ""]));
    assert_eq!(out.ready, [false, false, false, true, false, false]);
}

/// A line far longer than any calculation is treated as prose, so a pasted wall of text cannot stall the note.
#[test]
fn a_very_long_line_is_left_alone() {
    let long = format!("{}1 =", "1 + ".repeat(80));
    assert!(long.len() > 300);
    assert_eq!(worked(&long).results, column(&[""]));
    assert_eq!(worked("1 + 1 =\n\n\n").results.len(), 4);
}

/// A non-breaking space before a calculation (pasted text) must not panic the parser, and the calculation still works.
#[test]
fn a_non_breaking_space_before_a_calculation_is_survived() {
    assert_eq!(worked("cost?\u{00a0}2 + 3 =").results, column(&["5"]));
}

/// A subtotal on a list line is a total too: the next total does not count it again.
#[test]
fn a_total_on_a_list_line_is_not_counted_again() {
    let out = worked("1\n2\n- sum =\nsum =");
    assert_eq!(out.results, column(&["", "", "3", "3"]));
}

/// A name whose lower case is longer than the name (U+0130) does not eat the character after it.
#[test]
fn a_name_that_grows_in_lower_case_leaves_the_next_character() {
    assert_eq!(worked("\u{130}: 2\n\u{130}+1 =").results, column(&["", "3"]));
}

/// Thousands of distinct names all resolve; the last one is found.
#[test]
fn many_names_all_resolve() {
    let note: String = (0..20_000).map(|i| format!("n{i}: {i}\n")).chain(["n19999 + 1 ="].map(String::from)).collect();
    let out = worked(&note);
    assert_eq!(out.results.last().cloned().flatten().as_deref(), Some("20,000".replace(',', "").as_str()));
}

/// The longest of overlapping spaced names wins, whatever the case, and only at a word edge.
#[test]
fn name_lookup_prefers_the_longest_name_at_a_word_edge() {
    let cases = [
        ("o2: 1\no2 per tree: 5\nO2 PER TREE x 2 =", "10"),
        ("o2: 1\no2 per tree: 5\no2 x 2 =", "2"),
        ("tax: 3\ntaxes + 1 =", "1"),
        ("tax: 3\ntax_free + 1 =", "1"),
        ("tax: 3\n(tax)+1 =", "4"),
    ];
    for (note, last) in cases {
        let out = worked(note);
        assert_eq!(out.results.last().cloned().flatten().unwrap_or_default(), last, "{note}");
    }
}

/// A name of exactly 40 characters works, one of 41 is not a name, and 40 characters that lower-case to more still match.
#[test]
fn name_length_cap_is_forty_characters() {
    let forty = "a".repeat(40);
    assert_eq!(worked(&format!("{forty}: 7\n{forty} + 1 =")).results.last().cloned().flatten().as_deref(), Some("8"));
    let long = "a".repeat(41);
    assert_eq!(worked(&format!("{long}: 7\n{long} + 1 =")).values[0], None);
    let dotted = "\u{130}".repeat(40);
    assert_eq!(worked(&format!("{dotted}: 7\n{dotted} + 1 =")).results.last().cloned().flatten().as_deref(), Some("8"));
}
