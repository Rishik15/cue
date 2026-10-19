//! Tests of the snippet database: storing and listing, options, clashes, replacing, and the move from earlier file versions.

use super::*;
use crate::snippets::check::{MAX_BODY, MAX_NAME};
use crate::snippets::folders::folders;

fn db() -> Connection {
    open_memory()
}

fn draft(name: &str, body: &str) -> Draft {
    Draft { name: name.into(), body: body.into(), ..Draft::default() }
}

fn with_trigger(name: &str, trigger: &str) -> Draft {
    Draft { triggers: vec![trigger.into()], ..draft(name, "text") }
}

#[test]
fn text_with_quotes_and_sql_is_stored_as_data() {
    let conn = db();
    let nasty = "x'); DROP TABLE snippets; --";
    save(&conn, None, &draft(nasty, nasty)).unwrap();
    let all = list(&conn).unwrap();
    assert_eq!((all.len(), all[0].name.as_str(), all[0].body.as_str()), (1, nasty, nasty));
}

#[test]
fn limits_and_blank_snippets_are_refused() {
    let conn = db();
    assert!(save(&conn, None, &draft("  ", "text")).is_err());
    assert!(save(&conn, None, &draft("name", &"a".repeat(MAX_BODY + 1))).is_err());
    assert!(save(&conn, None, &draft(&"n".repeat(MAX_NAME + 1), "text")).is_err());
    assert!(list(&conn).unwrap().is_empty());
}

#[test]
fn most_used_snippets_come_first_and_edit_and_delete_work() {
    let conn = db();
    save(&conn, None, &draft("b", "two")).unwrap();
    save(&conn, None, &draft("a", "one")).unwrap();
    let b = list(&conn).unwrap().iter().find(|s| s.name == "b").unwrap().id;
    bump(&conn, b).unwrap();
    assert_eq!(list(&conn).unwrap()[0].name, "b");
    assert!(list(&conn).unwrap()[0].last_used > 0);
    save(&conn, Some(b), &draft("b2", "two!")).unwrap();
    assert_eq!(list(&conn).unwrap()[0].body, "two!");
    remove(&conn, b).unwrap();
    assert_eq!(list(&conn).unwrap().len(), 1);
}

#[test]
fn options_round_trip_and_a_trigger_cannot_be_used_twice() {
    let conn = db();
    let d = Draft {
        triggers: vec![":sig".into(), ":sign".into()],
        skip_apps: vec!["Slack.exe".into()],
        insert: Insert::Slow,
        ending: "-".into(),
        inside_word: true,
        ..draft("sig", "Best")
    };
    save(&conn, None, &d).unwrap();
    let s = &list(&conn).unwrap()[0];
    assert_eq!(
        (s.triggers.len(), s.insert, s.ending.as_str(), s.inside_word, s.skip_apps.as_slice()),
        (2, Insert::Slow, "-", true, ["slack".to_string()].as_slice())
    );
    assert!(save(&conn, None, &with_trigger("other", ":SIG")).is_err());
    assert!(save(&conn, Some(s.id), &d).is_ok(), "a snippet does not clash with itself");
    assert!(
        save(&conn, None, &Draft { enabled: false, ..with_trigger("off", ":sig") }).is_ok(),
        "a disabled snippet gives no trigger to clash"
    );
}

#[test]
fn a_first_version_file_is_brought_up_to_date_without_losing_anything() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE snippets (id INTEGER PRIMARY KEY, name TEXT NOT NULL, keyword TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0,
            folder TEXT NOT NULL DEFAULT '', mode TEXT NOT NULL DEFAULT 'auto', match_case INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1,
            paste INTEGER NOT NULL DEFAULT 0, skip_apps TEXT NOT NULL DEFAULT '');
         INSERT INTO snippets (name, keyword, body, folder, paste, skip_apps) VALUES ('a', ':a1', 'one', 'Work', 1, 'slack'), ('b', '', 'two', '', 0, '');
         PRAGMA user_version = 1;",
    )
    .unwrap();
    setup(&conn).unwrap();
    let all = list(&conn).unwrap();
    let a = all.iter().find(|s| s.name == "a").unwrap();
    assert_eq!(
        (a.triggers.as_slice(), a.insert, a.skip_apps.as_slice()),
        ([":a1".to_string()].as_slice(), Insert::Paste, ["slack".to_string()].as_slice())
    );
    assert_eq!(folders(&conn).unwrap()[0].name, "Work");
    assert_eq!(a.folder_id, Some(folders(&conn).unwrap()[0].id));
}

#[test]
fn replacing_a_snippet_that_is_gone_is_an_error_not_a_new_row() {
    let conn = db();
    assert!(save(&conn, Some(42), &draft("a", "one")).is_err());
    assert!(list(&conn).unwrap().is_empty());
}

#[test]
fn a_file_from_before_any_version_is_brought_to_the_latest_in_one_go() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE snippets (id INTEGER PRIMARY KEY, name TEXT NOT NULL, keyword TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0);
         INSERT INTO snippets (name, keyword, body) VALUES ('old', ':o', 'text');",
    )
    .unwrap();
    setup(&conn).unwrap();
    assert_eq!(conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), LATEST);
    let all = list(&conn).unwrap();
    assert_eq!((all.len(), all[0].triggers.as_slice()), (1, [":o".to_string()].as_slice()));
    setup(&conn).unwrap();
}

#[test]
fn the_index_listing_leaves_out_texts_longer_than_the_limit() {
    let conn = db();
    save(&conn, None, &draft("short", "abc")).unwrap();
    save(&conn, None, &draft("long", "é".repeat(10).as_str())).unwrap();
    let all = list_for_index(&conn, 8).unwrap();
    let body = |name: &str| all.iter().find(|s| s.name == name).unwrap().body.clone();
    assert_eq!((body("short").as_str(), body("long").as_str()), ("abc", ""), "the limit counts bytes");
}

#[test]
fn dropping_the_folder_prefix_writes_it_into_the_triggers() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE snippets (id INTEGER PRIMARY KEY, name TEXT NOT NULL, keyword TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0);
         INSERT INTO snippets (name, keyword, body) VALUES ('a', 'sig', 'one');
         PRAGMA user_version = 0;",
    )
    .unwrap();
    migrate(&conn, 2, |c| {
        c.execute_batch(
            "CREATE TABLE folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, enabled INTEGER NOT NULL DEFAULT 1,
                prefix TEXT NOT NULL DEFAULT '', only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '');
             INSERT INTO folders (name, prefix) VALUES ('F', ';');
             ALTER TABLE snippets ADD COLUMN folder_id INTEGER;
             ALTER TABLE snippets ADD COLUMN triggers TEXT NOT NULL DEFAULT '';
             UPDATE snippets SET folder_id = 1, triggers = keyword;",
        )
    })
    .unwrap();
    setup(&conn).unwrap();
    let has_prefix = conn.prepare("SELECT prefix FROM folders").is_ok();
    assert!(!has_prefix);
    assert_eq!(conn.query_row("SELECT triggers FROM snippets", [], |r| r.get::<_, String>(0)).unwrap(), ";sig");
}
