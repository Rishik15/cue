use rusqlite::Connection;

use super::*;
use crate::platform::Rect;

fn block(word: &str, pos: &str) -> Block {
    Block { word: word.into(), pos: pos.into(), senses: vec![], synonyms: vec![] }
}

fn pending(blocks: Vec<Block>) -> Pending {
    let none = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    Pending { language: "English".into(), blocks, pack: "dictionary-en", key: "k".into(), anchor: (none, none), source: 0 }
}

#[test]
fn the_word_heads_its_first_part_of_speech_and_again_when_it_changes() {
    let sections = pending(vec![block("polish", "verb"), block("polish", "noun"), block("Polish", "adjective")]).sections();
    let heads: Vec<_> = sections.iter().map(|s| s.heading.as_deref()).collect();
    assert_eq!(heads, [Some("polish"), None, Some("Polish")]);
}

/// A pack file in a temp folder: `words` are (key, JSON of blocks); None makes the file unreadable.
fn file(name: &str, words: Option<&[(&str, &str)]>) -> PathBuf {
    let path = std::env::temp_dir().join(format!("cue-lookup-{name}-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = Connection::open(&path).unwrap();
    let Some(words) = words else {
        conn.execute_batch("CREATE TABLE other (x)").unwrap();
        return path;
    };
    conn.execute_batch("CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL); CREATE TABLE meta (k TEXT, v TEXT)").unwrap();
    for (key, data) in words {
        conn.execute("INSERT INTO entries VALUES (?1, ?2)", [key, data]).unwrap();
    }
    path
}

const GO: &str = r#"[{"w":"go","p":"verb","s":[{"g":"To move."}]}]"#;

#[test]
fn find_word_reads_the_first_pack_that_knows_the_word() {
    let pack = catalog::find("dictionary-en").unwrap();
    let path = file("known", Some(&[("go", GO)]));
    let found = find_word(&[(pack, path.clone())], "go").unwrap().unwrap();
    assert_eq!((found.pack.id, found.blocks[0].word.as_str(), found.language.as_str()), (pack.id, "go", pack.name));
    assert!(find_word(&[(pack, path.clone())], "zzz").unwrap().is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn an_unreadable_pack_is_skipped_but_reported_when_nothing_else_has_the_word() {
    let pack = catalog::find("dictionary-en").unwrap();
    let (bad, good) = (file("bad", None), file("good", Some(&[("go", GO)])));
    assert!(find_word(&[(pack, bad.clone()), (pack, good.clone())], "go").unwrap().is_some());
    assert!(find_word(&[(pack, bad.clone()), (pack, good.clone())], "zzz").is_err());
    let _ = (std::fs::remove_file(bad), std::fs::remove_file(good));
}
