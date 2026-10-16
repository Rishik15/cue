use super::*;

fn word(text: &str) -> Saved {
    Saved {
        id: 0,
        word: text.into(),
        pos: "verb".into(),
        ipa: String::new(),
        gloss: "g".into(),
        example: String::new(),
        language: "English".into(),
        added: 5,
    }
}

#[test]
fn saves_lists_and_removes() {
    let dir = std::env::temp_dir().join(format!("cue-words-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let conn = open(&dir.join("w.db")).unwrap();
    save(&conn, "dictionary-en", "reach", &word("reach")).unwrap();
    save(&conn, "dictionary-en", "reach", &word("reach")).unwrap(); // saving twice keeps one row
    assert!(is_saved(&conn, "dictionary-en", "reach"));
    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);
    remove(&conn, all[0].id).unwrap();
    assert!(list(&conn).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
