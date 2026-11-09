//! File: text/tests/golden.rs
//! Purpose: Protect the held-out golden sets in `bench/golden/`: every model action has a file, every row is well formed, the cases the behaviour rules and the ticket name are all present, no case is also a prompt example, and the 39 tuning rows stay frozen.
//! Contents: the file table, a row parser, and tests of format, coverage, held-out-ness and the frozen slice.

use std::collections::HashSet;

use crate::actions::{task, Action};

const FILES: [(&str, &str); 12] = [
    ("correct", include_str!("../../../../bench/golden/correct.tsv")),
    ("make_shorter", include_str!("../../../../bench/golden/make_shorter.tsv")),
    ("make_formal", include_str!("../../../../bench/golden/make_formal.tsv")),
    ("make_friendly", include_str!("../../../../bench/golden/make_friendly.tsv")),
    ("tidy_speech", include_str!("../../../../bench/golden/tidy_speech.tsv")),
    ("bullet_list", include_str!("../../../../bench/golden/bullet_list.tsv")),
    ("summarize", include_str!("../../../../bench/golden/summarize.tsv")),
    ("simplify", include_str!("../../../../bench/golden/simplify.tsv")),
    ("explain_simply", include_str!("../../../../bench/golden/explain_simply.tsv")),
    ("write_reply", include_str!("../../../../bench/golden/write_reply.tsv")),
    ("find_tasks", include_str!("../../../../bench/golden/find_tasks.tsv")),
    ("translate_english", include_str!("../../../../bench/golden/translate_english.tsv")),
];
const FROZEN: &str = include_str!("../../../../bench/text_cases.tsv");
const TASK_CASES: &str = include_str!("../../../../bench/task_cases.tsv");

/// One golden row: kind, input, reference, tokens to keep, strings that must not appear, tag.
struct Row {
    kind: &'static str,
    input: String,
    want: String,
    tag: &'static str,
}

/// Turns the `\n` and `\uXXXX` escapes of a TSV cell into the characters. Input: the cell. Returns the text.
fn unescape(cell: &str) -> String {
    let mut out = String::new();
    let mut chars = cell.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('u') => {
                let code: String = chars.by_ref().take(4).collect();
                out.push(u32::from_str_radix(&code, 16).ok().and_then(char::from_u32).expect("a valid \\u escape"));
            }
            other => panic!("unknown escape {other:?}"),
        }
    }
    out
}

/// Parses a golden file. Input: its text. Returns its rows; panics on a malformed row so the test names it.
fn parse(text: &'static str) -> Vec<Row> {
    text.lines()
        .map(|line| {
            let cells: Vec<&'static str> = line.split('\t').collect();
            assert_eq!(cells.len(), 6, "six columns in {line:?}");
            Row { kind: cells[0], input: unescape(cells[1]), want: unescape(cells[2]), tag: cells[5] }
        })
        .collect()
}

/// Every action that uses the model has exactly one golden file, and nothing else is there.
#[test]
fn every_model_action_has_a_golden_file() {
    let wanted: HashSet<String> = Action::all().filter(|a| a.needs_model()).map(Action::id).collect();
    let have: HashSet<String> = FILES.iter().map(|(id, _)| id.to_string()).collect();
    assert_eq!(wanted, have);
}

/// Rows are well formed: a known kind, a reference for `fix`, an unchanged reference for `same`, and enough rows to mean something.
#[test]
fn rows_are_well_formed() {
    for (id, text) in FILES {
        let rows = parse(text);
        assert!(rows.len() >= 10, "{id} has too few rows");
        for row in &rows {
            assert!(["fix", "same", "keep", "inject"].contains(&row.kind), "{id}: kind {}", row.kind);
            assert!(row.kind != "fix" || !row.want.is_empty(), "{id}: fix row without a reference");
            assert!(row.kind != "same" || row.want == row.input, "{id}: same row changes the text");
        }
    }
}

/// Together the sets cover the cases the ticket names, and each set covers the behaviour rules.
#[test]
fn the_named_cases_are_all_covered() {
    let all: HashSet<&str> = FILES.iter().flat_map(|(_, t)| parse(t)).map(|r| r.tag).collect();
    for tag in [
        "negation", "tense", "quantity", "duplicate", "multilingual", "codeswitch", "selfcorrect", "empty", "long", "instruction", "fakerole",
        "delimiter", "hidden",
    ] {
        assert!(all.contains(tag) || (tag == "long" && all.contains("long_bad_unit")), "no case tagged {tag}");
    }
    for (id, text) in FILES {
        let tags: HashSet<&str> = parse(text).iter().map(|r| r.tag).collect();
        for needed in ["instruction", "fakerole", "delimiter", "empty"] {
            assert!(tags.contains(needed), "{id} lacks {needed}");
        }
        assert!(tags.contains("long") || tags.contains("long_bad_unit"), "{id} lacks a long input");
    }
}

/// The editing actions have the five behaviour-rule cases: a clean short sentence, a short question, a misspelled question, instruction-like text and a long text with one bad unit.
#[test]
fn editing_actions_cover_the_behaviour_rules() {
    for id in ["correct", "make_shorter", "make_formal", "make_friendly", "tidy_speech", "bullet_list", "simplify"] {
        let text = FILES.iter().find(|(name, _)| *name == id).expect("listed").1;
        let tags: HashSet<&str> = parse(text).iter().map(|r| r.tag).collect();
        for needed in ["short_question", "misspelled_question", "instruction"] {
            assert!(tags.contains(needed), "{id} lacks {needed}");
        }
    }
}

/// No golden input is also one of the prompt examples, so the sets stay held out.
#[test]
fn golden_inputs_are_not_prompt_examples() {
    let mut shots = HashSet::new();
    for action in Action::all() {
        for (said, _) in task(action).map(|t| t.shots).unwrap_or(&[]) {
            shots.insert(said.to_string());
        }
    }
    for (id, text) in FILES {
        for row in parse(text) {
            assert!(row.input.is_empty() || !shots.contains(&row.input), "{id}: {:?} is a prompt example", row.input);
        }
    }
}

/// The 39 rows the prompts were tuned on stay as they were, and none is reused as a golden input.
#[test]
fn the_tuning_rows_are_frozen_and_disjoint() {
    assert_eq!(FROZEN.lines().count(), 39);
    let tuned: HashSet<&str> = FROZEN.lines().chain(TASK_CASES.lines()).filter_map(|l| l.split('\t').nth(1)).collect();
    for (id, text) in FILES {
        for row in parse(text) {
            assert!(!tuned.contains(row.input.as_str()) || row.input.is_empty(), "{id}: {:?} is a tuning row", row.input);
        }
    }
}
