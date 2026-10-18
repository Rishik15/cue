use super::*;

fn word(text: &str, added: i64) -> Saved {
    Saved { id: 0, word: text.into(), pos: "verb".into(), gloss: "g".into(), example: String::new(), language: "English".into(), added }
}

fn db(name: &str) -> (Connection, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("cue-words-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    (open(&dir.join("w.db")).unwrap(), dir)
}

#[test]
fn records_lists_and_removes() {
    let (conn, dir) = db("basic");
    record(&conn, "dictionary-en", "reach", &word("reach", 5)).unwrap();
    record(&conn, "dictionary-en", "go", &word("go", 6)).unwrap();
    let all = list(&conn).unwrap();
    assert_eq!(all.iter().map(|w| w.word.as_str()).collect::<Vec<_>>(), ["go", "reach"]); // newest first
    remove(&conn, all[0].id).unwrap();
    assert_eq!(list(&conn).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn looking_a_word_up_again_moves_it_to_the_top_without_a_second_row() {
    let (conn, dir) = db("again");
    record(&conn, "dictionary-en", "reach", &word("reach", 5)).unwrap();
    record(&conn, "dictionary-en", "go", &word("go", 6)).unwrap();
    record(&conn, "dictionary-en", "reach", &word("reach", 9)).unwrap();
    let all = list(&conn).unwrap();
    assert_eq!(all.iter().map(|w| w.word.as_str()).collect::<Vec<_>>(), ["reach", "go"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn only_the_newest_rows_are_kept() {
    let (conn, dir) = db("cap");
    for i in 0..(KEEP + 3) {
        record(&conn, "dictionary-en", &format!("w{i}"), &word(&format!("w{i}"), i)).unwrap();
    }
    let all = list(&conn).unwrap();
    assert_eq!(all.len() as i64, KEEP);
    assert_eq!(all[0].word, format!("w{}", KEEP + 2));
    let _ = std::fs::remove_dir_all(dir);
}
