//! File: snippets/tests/policy.rs
//! Purpose: Protect that saving and typing agree: the page's rules decide both whether two saved triggers clash and which one fires while typing, over a real database and the real matcher.
//! Contents: scenario tests that save drafts under a chosen policy, build the matcher from what is stored and type the triggers.

use super::helpers::{draft, open_db};
use crate::platform::Typed;
use crate::snippets::expand::matcher::{Matcher, Tracker};
use crate::snippets::model::Mode;
use crate::snippets::policy::Policy;
use crate::snippets::store;

/// The id of the snippet that fires first while `text` is typed under `policy`, with everything in the database in force. Returns None when nothing fires.
fn first_fired(conn: &rusqlite::Connection, policy: &Policy, text: &str) -> Option<i64> {
    let matcher = Matcher::new(&store::effective_all(conn, policy).expect("live"));
    let mut tracker = Tracker::default();
    text.chars().find_map(|c| tracker.feed(Typed::Char(c), &matcher)).map(|hit| hit.id)
}

/// Under Immediate timing `a` fires before `ab` can be typed, so saving `ab` next to `a` is refused; under the default Auto both wait for a space and coexist. Save and typing agree.
#[test]
fn an_immediate_page_refuses_the_prefix_that_typing_would_shadow() {
    let immediate = Policy::with_mode(Mode::Immediate);
    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Short", &["a"], "x"), &immediate).expect("first");
    let Err(message) = store::save(&conn, None, &draft("Long", &["ab"], "y"), &immediate) else { panic!("ab was accepted") };
    assert!(message.contains("clashes with a"), "{message}");

    let (_dir, conn) = open_db();
    let auto = Policy::default();
    store::save(&conn, None, &draft("Short", &["a"], "x"), &auto).expect("first");
    store::save(&conn, None, &draft("Long", &["ab"], "y"), &auto).expect("coexist while both wait for a space");
    let long = store::list(&conn).expect("list").into_iter().find(|s| s.name == "Long").expect("long").id;
    assert_eq!(first_fired(&conn, &auto, "ab "), Some(long));
}

/// Under Delimiter timing `@@` and `@@p` both wait for the ending, so they save together and each fires by itself; under Auto (both symbol triggers fire at once) the pair is refused.
#[test]
fn a_delimiter_page_accepts_the_pair_that_auto_refuses() {
    let delimiter = Policy::with_mode(Mode::Delimiter);
    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Short", &["@@"], "x"), &delimiter).expect("first");
    store::save(&conn, None, &draft("Long", &["@@p"], "y"), &delimiter).expect("the longer one can still be typed");
    let long = store::list(&conn).expect("list").into_iter().find(|s| s.name == "Long").expect("long").id;
    assert_eq!(first_fired(&conn, &delimiter, "@@p "), Some(long));

    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Short", &["@@"], "x"), &Policy::default()).expect("first");
    assert!(store::save(&conn, None, &draft("Long", &["@@p"], "y"), &Policy::default()).is_err());
}
