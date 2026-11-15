//! File: dictionary/decide.rs
//! Purpose: What a Lookup shows for what was selected: the Card of a word, the Search window, or a Notice that says why there is nothing to show. One function over the Book, with no window or platform call, so every branch is tested here.
//! Contents: Decision — the outcome; decide — the decision; tests.
//! The rules: nothing selected (or only white space, hidden characters or `&nbsp;`, as UI Automation may report a collapsed selection) opens the Search window; so does a selection while no Pack is installed, because the Search window's no-pack state has the Download button and a Notice would be a dead end. A selection that is not a word or a short phrase (a sentence, digits, a link) and a word no Pack has are a Notice, never the Search window: the user meant that text, and a Notice leaves the keyboard in their app (the Card never takes it either), as the native card did. A Pack that cannot be read is a failure the caller reports.

use super::book::{Book, Entry};
use super::pack;

/// What a Lookup shows for a selection `S`.
#[derive(Debug, PartialEq)]
pub(super) enum Decision<S> {
    /// The Card of the selected word: the selection (to anchor the Card on) and its Entry.
    Card(S, Entry),
    /// The Search window, with nothing typed.
    Search,
    /// A short message on the pill, and no window.
    Notice(String),
    /// No Pack could be read: the reason, for `status::report`.
    Failed(String),
}

/// Decides what a Lookup shows. Input: the selection, whose text is as UI Automation or the copy gave it (None, or blank once hidden characters and non-breaking spaces are cleaned, when nothing is selected), and the installed Packs. Return: the Decision, which hands a Card its selection back.
pub(super) fn decide<S: AsRef<str>>(selection: Option<S>, book: &Book) -> Decision<S> {
    let Some((selection, text)) = selection
        .map(|s| {
            let text = crate::sanitize::clean_selection(s.as_ref());
            (s, text)
        })
        .filter(|(_, t)| !t.is_empty())
    else {
        return Decision::Search;
    };
    if book.is_empty() {
        return Decision::Search;
    }
    let Some(key) = pack::normalize(&text) else { return Decision::Notice("Select one word".into()) };
    match book.lookup(&text) {
        Ok(Some(entry)) => Decision::Card(selection, entry),
        Ok(None) => Decision::Notice(format!("No definition for {key}")),
        Err(why) => Decision::Failed(why),
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{decide, Book, Decision};

    /// A Book over one English Pack holding "serene" and one damaged entry, "serz". Return: the Book and the folder it lives in.
    fn book() -> (Book, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("en.db");
        Connection::open(&path)
            .expect("create")
            .execute_batch(
                r#"CREATE TABLE meta (k TEXT PRIMARY KEY, v TEXT); INSERT INTO meta VALUES ('language', 'English');
                   CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL);
                   INSERT INTO entries VALUES ('serene', '[{"w":"serene","p":"adjective","s":[{"g":"Calm."}]}]');
                   INSERT INTO entries VALUES ('serz', 'not json');"#,
            )
            .expect("fill");
        (Book::new(vec![("en", "English", path)]), dir)
    }

    /// Nothing selected, or only white space and hidden characters, opens the Search window.
    #[test]
    fn nothing_selected_opens_the_search_window() {
        let (book, _dir) = book();
        for selection in [None, Some(""), Some("  \n\t "), Some("\u{A0}&nbsp;\u{200B}")] {
            assert_eq!(decide(selection, &book), Decision::<&str>::Search, "{selection:?}");
        }
    }

    /// A selection with no Pack installed opens the Search window, whose no-pack state offers the download.
    #[test]
    fn a_selection_without_a_pack_opens_the_search_window() {
        assert_eq!(decide(Some("serene"), &Book::new(Vec::new())), Decision::Search);
    }

    /// A word a Pack has is a Card with its Entry, whatever its case and the punctuation round it.
    #[test]
    fn a_word_the_pack_has_is_a_card() {
        let (book, _dir) = book();
        let Decision::Card(selection, entry) = decide(Some(" Serene, "), &book) else { panic!("not a card") };
        assert_eq!((selection, entry.key.as_str(), entry.blocks[0].pos.as_str()), (" Serene, ", "serene", "adjective"));
    }

    /// A selection that is not a word, and a word no Pack has, are Notices naming the reason.
    #[test]
    fn a_selection_that_is_not_a_known_word_is_a_notice() {
        let (book, _dir) = book();
        assert_eq!(decide(Some("one two three four five"), &book), Decision::Notice("Select one word".into()));
        assert_eq!(decide(Some("42"), &book), Decision::Notice("Select one word".into()));
        assert_eq!(decide(Some("Zebra"), &book), Decision::Notice("No definition for zebra".into()));
    }

    /// A damaged Pack is a failure to report, never "No definition".
    #[test]
    fn a_damaged_pack_is_a_failure() {
        let (book, _dir) = book();
        assert!(matches!(decide(Some("serz"), &book), Decision::Failed(why) if why.starts_with("The dictionary file is damaged")));
    }
}
