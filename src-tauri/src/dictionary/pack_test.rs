use rusqlite::Connection;

use super::*;

fn pack() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL);
           CREATE TABLE forms (form TEXT PRIMARY KEY, key TEXT NOT NULL, pos TEXT NOT NULL);
           INSERT INTO entries VALUES ('reach', '[{"w":"reach","p":"verb","i":"/ɹiːtʃ/","s":[{"g":"To stretch out an arm.","e":"He reached over."}],"y":["extend"]}]');
           INSERT INTO entries VALUES ('dog', '[{"w":"dog","p":"noun","s":[{"g":"An animal."}]}]');
           INSERT INTO entries VALUES ('stop', '[{"w":"stop","p":"verb","s":[{"g":"To cease."}]}]');
           INSERT INTO entries VALUES ('city', '[{"w":"city","p":"noun","s":[{"g":"A large town."}]}]');
           INSERT INTO entries VALUES ('go', '[{"w":"go","p":"noun","s":[{"g":"A turn."}]},{"w":"go","p":"verb","s":[{"g":"To move."}]}]');
           INSERT INTO forms VALUES ('went', 'go', 'verb');"#,
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
}

#[test]
fn refuses_what_is_not_a_word() {
    for bad in ["", "   ", "12345", "a+b", "one two three four five", "https://example.com", &"a".repeat(60)] {
        assert_eq!(normalize(bad), None, "{bad}");
    }
}

#[test]
fn finds_a_word() {
    let found = define(&pack(), "reach").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].ipa.as_deref(), Some("/ɹiːtʃ/"));
    assert_eq!(found[0].senses[0].example.as_deref(), Some("He reached over."));
}

#[test]
fn an_inflected_form_finds_its_base_word() {
    let conn = pack();
    for (form, base) in
        [("reached", "reach"), ("stopped", "stop"), ("cities", "city"), ("dogs", "dog"), ("reaching", "reach"), ("went", "go")]
    {
        assert_eq!(define(&conn, form).unwrap()[0].word, base, "{form}");
    }
}

#[test]
fn the_part_of_speech_of_the_inflection_comes_first() {
    assert_eq!(define(&pack(), "went").unwrap()[0].pos, "verb");
    assert_eq!(define(&pack(), "go").unwrap()[0].pos, "noun");
}

#[test]
fn possessive_falls_back_to_the_word() {
    assert_eq!(define(&pack(), "dog's").unwrap()[0].word, "dog");
}

#[test]
fn unknown_word_is_empty_and_damage_is_an_error() {
    let conn = pack();
    assert!(define(&conn, "zzz").unwrap().is_empty());
    conn.execute("INSERT INTO entries VALUES ('bad', 'not json')", []).unwrap();
    assert!(define(&conn, "bad").is_err());
}
