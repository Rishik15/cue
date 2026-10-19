use rusqlite::Connection;

use super::*;

fn pack() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL);
           INSERT INTO entries VALUES ('reach', '[{"w":"reach","p":"verb","s":[{"g":"To stretch out an arm.","e":"He reached over."},{"g":"To arrive at."}]}]');
           INSERT INTO entries VALUES ('went', '[{"w":"went","p":"verb","s":[{"g":"simple past of go"}]}]');
           INSERT INTO entries VALUES ('polish', '[{"w":"polish","p":"verb","s":[{"g":"To make shiny."}]},{"w":"Polish","p":"adjective","s":[{"g":"Of Poland."}]}]');"#,
    )
    .unwrap();
    conn
}

#[test]
fn normalizes_selections() {
    assert_eq!(normalize("  Reach, "), Some("reach".into()));
    assert_eq!(normalize("“par à-coups”"), Some("par à-coups".into()));
    assert_eq!(normalize("don\u{2019}t"), Some("don't".into()));
    assert_eq!(normalize("look   up\n"), Some("look up".into()));
    assert_eq!(normalize("cafe\u{301}"), Some("cafe\u{301}".into()));
}

#[test]
fn refuses_what_is_not_a_word() {
    for bad in ["", "   ", "12345", "a+b", "one two three four five", "https://example.com", &"a".repeat(60)] {
        assert_eq!(normalize(bad), None, "{bad}");
    }
}

#[test]
fn finds_a_word_with_its_meanings_and_examples() {
    let found = define(&pack(), "reach").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].senses.len(), 2);
    assert_eq!(found[0].senses[0].example.as_deref(), Some("He reached over."));
}

#[test]
fn an_inflected_form_is_an_entry_of_its_own() {
    assert_eq!(define(&pack(), "went").unwrap()[0].senses[0].gloss, "simple past of go");
}

#[test]
fn one_key_keeps_every_word_written_that_way() {
    let found = define(&pack(), "polish").unwrap();
    assert_eq!(found.iter().map(|b| b.word.as_str()).collect::<Vec<_>>(), ["polish", "Polish"]);
}

#[test]
fn unknown_word_is_empty_and_damage_is_an_error() {
    let conn = pack();
    assert!(define(&conn, "zzz").unwrap().is_empty());
    conn.execute("INSERT INTO entries VALUES ('bad', 'not json')", []).unwrap();
    assert!(define(&conn, "bad").is_err());
}
