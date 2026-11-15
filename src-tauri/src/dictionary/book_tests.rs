//! File: dictionary/book_tests.rs
//! Purpose: Protect the dictionary's two questions, tested only through `Book`: what a typed prefix finds (order, limit, damaged data, several packs, hostile queries) and what a word's entry is; plus the prefix arithmetic with property tests.
//! Contents: a pack file builder; tests of `Book::search`, `Book::lookup`, `clean_query` and `successor`.

use std::path::PathBuf;

use proptest::prelude::*;
use rusqlite::Connection;

use super::book::{clean_query, fold_query, successor, Book, MAX_HITS, MAX_QUERY};

/// A pack file holding `words` (each with one noun sense named after it), plus one damaged entry, `key` "serz" with data that is not JSON. Return: its path (the folder lives as long as the returned guard).
fn pack_of(dir: &tempfile::TempDir, file: &str, language: &str, words: &[&str]) -> PathBuf {
    let path = dir.path().join(file);
    let conn = Connection::open(&path).expect("create");
    conn.execute_batch(&format!(
        "CREATE TABLE meta (k TEXT PRIMARY KEY, v TEXT); INSERT INTO meta VALUES ('language', '{language}');
         CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL); INSERT INTO entries VALUES ('serz', 'not json');"
    ))
    .expect("schema");
    for word in words {
        let data = format!(r#"[{{"w":"{word}","p":"noun","s":[{{"g":"meaning of {word}","e":"use {word}"}}]}}]"#);
        conn.execute("INSERT INTO entries VALUES (?1, ?2)", [word, data.as_str()]).expect("entry");
    }
    path
}

/// A book over one English pack of `words`.
fn book(words: &[&str]) -> (Book, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("dir");
    let path = pack_of(&dir, "en.db", "English", words);
    (Book::new(vec![("en", "English", path)]), dir)
}

/// A prefix finds exactly the words that start with it, alphabetically, up to the limit, folded to lower case; the first meaning rides along.
#[test]
fn a_prefix_finds_the_words_that_start_with_it_in_order() {
    let (book, _dir) = book(&["series", "serene", "serif", "sea", "serenity", "s", "zebra"]);
    let keys = |q: &str, n: usize| book.search(q, n).expect("search").into_iter().map(|h| h.key).collect::<Vec<_>>();
    assert_eq!(keys("ser", 10), ["serene", "serenity", "series", "serif"]);
    assert_eq!(keys("  SERe", 10), ["serene", "serenity"]);
    assert_eq!(keys("ser", 2), ["serene", "serenity"]);
    assert_eq!(keys("s", 3), ["s", "sea", "serene"]);
    assert_eq!(keys("q", 5), Vec::<String>::new());
    let hit = &book.search("serif", 1).expect("one")[0];
    assert_eq!((hit.word.as_str(), hit.pos.as_str(), hit.gloss.as_str()), ("serif", "noun", "meaning of serif"));
}

/// A damaged entry is left out of the results, an empty query finds nothing and the limit never passes `MAX_HITS`.
#[test]
fn damaged_entries_and_empty_queries_find_nothing_and_the_limit_is_capped() {
    let words: Vec<String> = (0..40).map(|i| format!("word{}", char::from(b'a' + (i % 26) as u8)) + &"x".repeat(i / 26)).collect();
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let (book, _dir) = book(&refs);
    assert!(book.search("serz", 5).expect("damaged").is_empty());
    assert!(book.search("   ", 5).expect("blank").is_empty());
    assert_eq!(book.search("word", 1000).expect("wide").len(), MAX_HITS);
}

/// A query that is too long or has anything but letters is refused before any SQL runs, and the quote and percent signs an injection would use never reach the database.
#[test]
fn hostile_queries_are_refused() {
    let (book, _dir) = book(&["serene"]);
    for bad in ["ser'; DROP TABLE entries; --", "100%", "a_b", "ser\u{0}", "x=1", &"a".repeat(MAX_QUERY + 1)] {
        assert!(book.search(bad, 5).is_err(), "{bad:?}");
    }
    assert_eq!(book.search("serene", 5).expect("table is still there").len(), 1);
}

/// Several packs merge by key; one that cannot be read does not hide the others, and only when none can be read is it an error.
#[test]
fn several_packs_merge_and_a_broken_one_does_not_hide_the_rest() {
    let dir = tempfile::tempdir().expect("dir");
    let a = pack_of(&dir, "a.db", "English", &["serene", "serif"]);
    let b = pack_of(&dir, "b.db", "German", &["serene", "serviette"]);
    let broken = dir.path().join("broken.db");
    std::fs::write(&broken, b"not a database").expect("write");
    let both = Book::new(vec![("en", "English", a), ("de", "German", b)]);
    let keys: Vec<_> = both.search("ser", 10).expect("merged").into_iter().map(|h| h.key).collect();
    assert_eq!(keys, ["serene", "serif", "serviette"]);
    let with_broken = Book::new(vec![("x", "X", broken.clone()), ("en", "English", dir.path().join("a.db"))]);
    assert_eq!(with_broken.search("ser", 10).expect("one good pack").len(), 2);
    assert!(Book::new(vec![("x", "X", broken)]).search("ser", 10).is_err());
    assert!(Book::new(Vec::new()).search("ser", 10).expect("no pack").is_empty());
}

/// A lookup normalises its word, names the pack's language, and finds nothing for text that is not a word.
#[test]
fn a_lookup_normalises_and_names_the_language() {
    let (book, _dir) = book(&["serene"]);
    let entry = book.lookup("  Serene, ").expect("lookup").expect("found");
    assert_eq!((entry.key.as_str(), entry.language.as_str(), entry.pack, entry.blocks.len()), ("serene", "English", "en", 1));
    assert_eq!(book.lookup("absent").expect("none"), None);
    assert_eq!(book.lookup("x = y").expect("not a word"), None);
}

/// A query folds to a lower-case prefix with single spaces; blank is none.
#[test]
fn a_query_folds_to_a_prefix() {
    assert_eq!(clean_query("  Ice   Cr").unwrap().as_deref(), Some("ice cr"));
    assert_eq!(clean_query("don\u{2019}t").unwrap().as_deref(), Some("don't"));
    assert_eq!(clean_query("ice ").unwrap().as_deref(), Some("ice "));
    assert_eq!(clean_query(" \t ").unwrap(), None);
}

proptest! {
    /// A string starts with a prefix exactly when it lies in the range from the prefix up to its successor, so one range read on the key index finds all and only the prefix matches.
    #[test]
    fn the_successor_bounds_exactly_the_prefix_range(prefix in "[a-z\u{e9}\u{4e2d}'-]{1,6}", word in "[a-z\u{e9}\u{4e2d}'-]{0,8}") {
        let above = successor(&prefix).expect("a successor");
        prop_assert!(above > prefix);
        prop_assert_eq!(word.starts_with(&prefix), word >= prefix && word < above);
    }

    /// Whatever is typed, a cleaned query is short, lower case and made of allowed characters only, and the cleaner never panics.
    #[test]
    fn a_cleaned_query_is_always_safe(text in "\\PC{0,60}") {
        if let Ok(Some(q)) = clean_query(&text) {
            prop_assert!(q.chars().count() <= MAX_QUERY);
            let allowed = |c: char| c.is_alphabetic() || ('\u{300}'..='\u{36F}').contains(&c) || matches!(c, ' ' | '\'' | '-');
            prop_assert!(q.chars().all(allowed));
            prop_assert_eq!(q.to_lowercase(), q);
        }
    }
}

/// The fold agrees with the page's (`fold` in `src/lookup/nav.ts`) on every case of the fixture both read: U+FEFF stays, U+0085 is white space, runs collapse, curly apostrophes and case fold.
#[test]
fn the_fold_agrees_with_the_pages_on_the_shared_cases() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!("../../../src/lookup/fold-cases.json")).expect("fixture");
    assert!(cases.len() > 10);
    for case in cases {
        let (input, folded) = (case["input"].as_str().expect("input"), case["folded"].as_str().expect("folded"));
        assert_eq!(fold_query(input), folded, "{input:?}");
    }
}
