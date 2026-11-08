//! File: dictionary/tests.rs
//! Purpose: Protect looking a word up: what counts as a word, reading a pack file, the saved lookups list (newest first, one row per word, capped) and the popup press rules.
//! Contents: tests of `pack::normalize`, `pack::define`, `words` and `phase::press_action`.

use std::time::Duration;

use rusqlite::Connection;

use super::pack::{self, Block, Sense};
use super::phase::{press_action, Action, IDLE, LOOKING, OPEN};
use super::words::{self, Saved};

/// A selection becomes a lower-case key only when it is a word or a short phrase.
#[test]
fn selections_that_are_words_become_keys_and_the_rest_do_not() {
    for (selection, key) in [
        ("  Hello, ", "hello"),
        ("\"Don\u{2019}t\"", "don't"),
        ("well-known", "well-known"),
        ("ice  cream", "ice cream"),
        ("Caf\u{e9}", "caf\u{e9}"),
        ("e\u{301}t\u{e9}", "e\u{301}t\u{e9}"),
    ] {
        assert_eq!(pack::normalize(selection).as_deref(), Some(key), "{selection:?}");
    }
    let sentence = "one two three four five";
    let long = "a".repeat(49);
    for none in ["", "   ", "42", "a1", "x = y", "https://x.io", sentence, long.as_str()] {
        assert_eq!(pack::normalize(none), None, "{none:?}");
    }
}

/// A pack built as the build script does it (short keys, inflections as entries) answers with the blocks, and a missing word, a damaged entry or a non-pack file fail readably.
#[test]
fn a_pack_file_answers_lookups_and_reports_damage() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("en.db");
    let build = Connection::open(&path).expect("create");
    build
        .execute_batch(
            r#"CREATE TABLE meta (k TEXT PRIMARY KEY, v TEXT); INSERT INTO meta VALUES ('language', 'English');
               CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL);
               INSERT INTO entries VALUES ('go', '[{"w":"go","p":"verb","s":[{"g":"to move","e":"go home"},{"g":"to leave"}],"y":["depart"]}]');
               INSERT INTO entries VALUES ('went', '[{"w":"went","p":"verb","s":[{"g":"past of go"}]}]');
               INSERT INTO entries VALUES ('broken', 'not json');"#,
        )
        .expect("fill");
    drop(build);
    let conn = pack::open(&path).expect("open read only");
    assert_eq!(pack::language(&conn).as_deref(), Some("English"));
    let go = pack::define(&conn, "go").expect("go");
    let sense = |gloss: &str, example: Option<&str>| Sense { gloss: gloss.into(), example: example.map(String::from) };
    assert_eq!(
        go,
        [Block {
            word: "go".into(),
            pos: "verb".into(),
            senses: vec![sense("to move", Some("go home")), sense("to leave", None)],
            synonyms: vec!["depart".into()]
        }]
    );
    assert_eq!(pack::define(&conn, "went").expect("went")[0].senses[0].gloss, "past of go");
    assert!(pack::define(&conn, "zzz").expect("missing").is_empty());
    assert!(pack::define(&conn, "broken").expect_err("damaged").starts_with("The dictionary file is damaged"));
    assert!(conn.execute("DELETE FROM entries", []).is_err(), "a pack is read only");
    assert!(pack::open(&dir.path().join("absent.db")).is_err());
}

/// One lookup row for a word. Inputs: the word and when it was looked up. Returns the row.
fn saved(word: &str, added: i64) -> Saved {
    Saved {
        id: 0,
        word: word.into(),
        pos: "noun".into(),
        gloss: format!("meaning of {word}"),
        example: String::new(),
        language: "English".into(),
        added,
    }
}

/// Looking a word up again moves it to the top without a second row, the same word in another pack is separate, and removing takes it off.
#[test]
fn looked_up_words_list_newest_first_one_row_per_word() {
    let dir = tempfile::tempdir().expect("dir");
    let conn = words::open(&dir.path().join("words.db")).expect("open");
    for (pack_id, word, added) in [("en", "apple", 1), ("en", "pear", 2), ("en", "apple", 3), ("pl", "apple", 4)] {
        words::record(&conn, pack_id, word, &saved(word, added)).expect("record");
    }
    let list = words::list(&conn).expect("list");
    assert_eq!(list.iter().map(|w| (w.word.as_str(), w.added)).collect::<Vec<_>>(), [("apple", 4), ("apple", 3), ("pear", 2)]);
    words::remove(&conn, list[2].id).expect("remove");
    assert_eq!(words::list(&conn).expect("list").len(), 2);
}

/// Only the newest 500 lookups are kept.
#[test]
fn the_lookup_list_keeps_only_the_newest_five_hundred() {
    let dir = tempfile::tempdir().expect("dir");
    let conn = words::open(&dir.path().join("words.db")).expect("open");
    for i in 0..505 {
        let word = format!("word{i}");
        words::record(&conn, "en", &word, &saved(&word, i)).expect("record");
    }
    let list = words::list(&conn).expect("list");
    assert_eq!((list.len(), list[0].word.as_str(), list[499].word.as_str()), (500, "word504", "word5"));
}

/// A press starts a lookup when idle, is ignored as a double tap right after, and closes the popup later.
#[test]
fn the_shortcut_starts_ignores_a_double_tap_or_closes() {
    let (soon, later) = (Duration::from_millis(100), Duration::from_millis(900));
    assert_eq!(press_action(IDLE, soon), Action::Start);
    assert_eq!(press_action(LOOKING, soon), Action::Ignore);
    assert_eq!(press_action(OPEN, soon), Action::Ignore);
    assert_eq!(press_action(LOOKING, later), Action::Close);
    assert_eq!(press_action(OPEN, later), Action::Close);
}
