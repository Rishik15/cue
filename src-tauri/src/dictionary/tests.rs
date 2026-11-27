//! File: dictionary/tests.rs
//! Purpose: Protect looking a word up: what counts as a word, reading a Pack, Looked Up (newest first, one row per word, capped) and what a lookup opens.
//! Contents: tests of `pack::normalize`, `pack::define`, `words`, `proceed` and the threading of `dict_download`.

use rusqlite::Connection;

use super::book::Entry;
use super::pack::{self, Block, Sense};
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
               INSERT INTO entries VALUES ('broken', 'not json');
               INSERT INTO entries VALUES ('private', '"secret-sentinel"');"#,
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
    assert!(!pack::define(&conn, "private").expect_err("wrong shape").contains("secret-sentinel"), "an error never quotes entry text");
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
    let mut conn = words::open(&dir.path().join("words.db")).expect("open");
    for (pack_id, word, added) in [("en", "apple", 1), ("en", "pear", 2), ("en", "apple", 3), ("pl", "apple", 4)] {
        words::record(&mut conn, pack_id, word, &saved(word, added)).expect("record");
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
    let mut conn = words::open(&dir.path().join("words.db")).expect("open");
    for i in 0..505 {
        let word = format!("word{i}");
        words::record(&mut conn, "en", &word, &saved(&word, i)).expect("record");
    }
    let list = words::list(&conn).expect("list");
    assert_eq!((list.len(), list[0].word.as_str(), list[499].word.as_str()), (500, "word504", "word5"));
}

/// A lookup whose word's window left the front before anything showed opens nothing, even when the failed read looks like nothing selected (which would open the search window and take the keyboard from the app the user went to); a cancelled lookup opens nothing either.
#[test]
fn a_lookup_whose_window_left_the_front_opens_nothing() {
    use super::flow::proceed;
    use super::lookup::{Picked, Selected};
    let word = || Picked::Word(Selected { text: "serene".into(), lines: Vec::new() });
    assert!(proceed(Picked::Nothing, true, false).is_none());
    assert!(proceed(word(), true, false).is_none());
    assert!(proceed(Picked::Nothing, false, true).is_none());
    assert!(matches!(proceed(Picked::Nothing, true, true), Some(Picked::Nothing)));
    assert!(matches!(proceed(word(), true, true), Some(Picked::Word(_))));
}

/// The no-pack button's command is asynchronous and works on a blocking worker: a synchronous command runs on the main thread, where building the settings webview deadlocks on Windows. The command itself needs a running app, so its declaration is what is checked.
#[test]
fn the_download_command_never_runs_on_the_main_thread() {
    let source = include_str!("commands.rs");
    let body = source.split("pub async fn dict_download").nth(1).expect("dict_download is async");
    let body = &body[..body.find("\n}").expect("end of the command")];
    assert!(body.contains("crate::snippets::blocking(move ||"));
    assert!(body.find("blocking(").expect("worker") < body.find("open_settings_at").expect("opens the settings"));
}

/// A file of an earlier version (with the `ipa` column and rows, no stamp) keeps every row and their order once opened, loses the column and is stamped; a new file never has the column.
#[test]
fn a_words_file_from_an_earlier_version_loses_its_ipa_column() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("words.db");
    let old = Connection::open(&path).expect("old");
    old.execute_batch(
        "CREATE TABLE words (id INTEGER PRIMARY KEY, key TEXT NOT NULL, pack TEXT NOT NULL, word TEXT NOT NULL, pos TEXT NOT NULL,
            ipa TEXT NOT NULL DEFAULT '', gloss TEXT NOT NULL, example TEXT NOT NULL, language TEXT NOT NULL, added INTEGER NOT NULL, UNIQUE (pack, key));
         INSERT INTO words (key, pack, word, pos, ipa, gloss, example, language, added) VALUES ('a', 'en', 'a', 'noun', 'x', 'g', '', 'English', 1), ('b', 'en', 'b', 'noun', '', 'g', '', 'English', 2);",
    )
    .expect("old rows");
    drop(old);
    let ipa = |conn: &Connection| {
        conn.query_row("SELECT count(*) FROM pragma_table_info('words') WHERE name = 'ipa'", [], |r| r.get::<_, i64>(0)).expect("columns")
    };
    let version = |conn: &Connection| conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).expect("version");
    let conn = words::open(&path).expect("open");
    assert_eq!((ipa(&conn), version(&conn)), (0, 1));
    assert_eq!(words::list(&conn).expect("list").iter().map(|w| w.word.as_str()).collect::<Vec<_>>(), ["b", "a"]);
    drop(conn);
    let again = words::open(&path).expect("reopen");
    assert_eq!((ipa(&again), version(&again), words::list(&again).expect("list").len()), (0, 1, 2));
    let fresh = words::open(&dir.path().join("new.db")).expect("fresh");
    assert_eq!((ipa(&fresh), version(&fresh)), (0, 1));
}

/// A revisit within the same second still moves the word to the top (order is the row id, not the stored seconds), one row per word, and the prune keeps the newest rows across revisits.
#[test]
fn a_revisit_in_the_same_second_moves_to_the_top() {
    let dir = tempfile::tempdir().expect("dir");
    let mut conn = words::open(&dir.path().join("words.db")).expect("open");
    for word in ["alpha", "beta", "alpha"] {
        words::record(&mut conn, "en", word, &saved(word, 7)).expect("record");
    }
    let names = |rows: Vec<Saved>| rows.into_iter().map(|w| w.word).collect::<Vec<_>>();
    assert_eq!(names(words::list(&conn).expect("list")), ["alpha", "beta"]);
    assert_eq!(words::recent(&conn, 5).expect("recent").into_iter().map(|(w, _)| w).collect::<Vec<_>>(), ["alpha", "beta"]);
    for i in 0..505 {
        let word = format!("word{i}");
        words::record(&mut conn, "en", &word, &saved(&word, 7)).expect("record");
    }
    words::record(&mut conn, "en", "word10", &saved("word10", 7)).expect("revisit");
    let list = words::list(&conn).expect("list");
    assert_eq!((list.len(), list[0].word.as_str(), list[1].word.as_str(), list[499].word.as_str()), (500, "word10", "word504", "word5"));
}

/// `dict_seen` is asynchronous and writes on a blocking worker: a synchronous command runs on the main thread, where opening `words.db` can stall the UI. The command needs a running app (no behavioural test is possible in `cargo test`), so its declaration is what is checked.
#[test]
fn dict_seen_is_asynchronous_and_works_on_a_blocking_worker() {
    let source = include_str!("commands.rs");
    let body = source.split("pub async fn dict_seen").nth(1).expect("dict_seen is async");
    let body = &body[..body.find("\n}").expect("end of the command")];
    assert!(body.contains("crate::snippets::blocking(move ||"));
    assert!(body.find("blocking(").expect("worker") < body.find("saved::record").expect("records"));
}

/// The Search window's Recent list reads only as many rows as it shows, newest first.
#[test]
fn recent_words_are_the_newest_few_of_many() {
    let dir = tempfile::tempdir().expect("dir");
    let mut conn = words::open(&dir.path().join("words.db")).expect("open");
    for i in 0..20 {
        let word = format!("w{i:02}");
        words::record(&mut conn, "en", &word, &saved(&word, i)).expect("record");
    }
    let recent = words::recent(&conn, 8).expect("recent");
    assert_eq!(recent.len(), 8);
    assert_eq!((recent[0].0.as_str(), recent[7].0.as_str()), ("w19", "w12"));
}

/// A lookup that finishes after its window closed is not recorded.
#[test]
fn a_lookup_finishing_after_its_window_closed_records_nothing() {
    let mut recorded = false;
    assert!(!super::commands::record_while_open("dictionary-7", || recorded = true));
    assert!(!recorded);
}

/// A step that failed closes the lookup and says so; one that worked closes nothing.
#[test]
fn a_failed_window_step_closes_the_lookup() {
    let mut closed = 0;
    assert!(!super::window::closing_on_err(Err::<(), _>("no thread"), "start the watchdog", || closed += 1));
    assert!(super::window::closing_on_err(Ok::<_, String>(()), "show the window", || closed += 1));
    assert_eq!(closed, 1);
}

#[test]
fn a_late_search_of_an_older_window_keeps_the_newer_windows_book() {
    let mut kept = None;
    let newer = super::window::cached(&mut kept, 2, || "two");
    let late = super::window::cached(&mut kept, 1, || "one");
    assert_eq!(*late, "one");
    assert!(std::sync::Arc::ptr_eq(&newer, &super::window::cached(&mut kept, 2, || "again")));
    let next = super::window::cached(&mut kept, 3, || "three");
    assert_eq!(*next, "three");
    assert_eq!(kept.map(|(id, _)| id), Some(3));
}

/// Releasing a window's book drops its own and never a newer window's.
#[test]
fn releasing_an_older_window_keeps_the_newer_windows_book() {
    let mut kept = None;
    let _ = super::window::cached(&mut kept, 2, || "two");
    super::window::drop_kept(&mut kept, 1);
    assert_eq!(kept.as_ref().map(|(id, _)| *id), Some(2));
    super::window::drop_kept(&mut kept, 2);
    assert!(kept.is_none());
}

/// A page already closed that reports a word it showed takes nothing, so an entry held for the same word by a newer window is still there; the open window's own report takes it.
#[test]
fn a_stale_pages_seen_leaves_the_held_entry_for_the_open_window() {
    let _serial = super::serial_test();
    let id = {
        let mut state = super::state();
        let id = state.begin(std::time::Instant::now()).expect("idle");
        let work = crate::platform::Rect { left: 0, top: 0, right: 100, bottom: 100 };
        let session = super::state::Session {
            id,
            mode: super::state::Mode::Card,
            anchor: None,
            work,
            scale: 1.0,
            source: 0,
            placed: None,
            open: None,
        };
        assert!(state.publish(id, session).is_some());
        id
    };
    let (open, stale) = (super::label::label(id), super::label::label(id - 1));
    let entry = Entry { key: "stale-race-word".into(), language: "en".into(), pack: "en", blocks: Vec::new() };
    super::saved::hold(&open, entry);
    assert!(super::commands::seen_entry(&stale, "stale-race-word").is_none());
    assert!(super::commands::seen_entry(&open, "stale-race-word").is_some());
    assert!(super::commands::seen_entry(&open, "stale-race-word").is_none());
    super::state().close();
}

/// The hook's question never waits for the state lock, follows Card open and close, and ignores the Search window.
#[test]
fn card_up_does_not_wait_for_the_state_lock() {
    use super::state::{Mode, Session};
    let _serial = super::serial_test();
    let open = |mode| {
        let mut state = super::state();
        let id = state.begin(std::time::Instant::now()).expect("idle");
        let work = crate::platform::Rect { left: 0, top: 0, right: 100, bottom: 100 };
        let session = Session { id, mode, anchor: None, work, scale: 1.0, source: 0, placed: None, open: None };
        assert!(state.publish(id, session).is_some());
    };
    open(Mode::Search);
    assert!(!super::card_up());
    super::state().close();
    open(Mode::Card);
    let held = super::state();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || tx.send(super::card_up()).expect("send"));
    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(1)), Ok(true));
    drop(held);
    super::state().close();
    assert!(!super::card_up());
}
