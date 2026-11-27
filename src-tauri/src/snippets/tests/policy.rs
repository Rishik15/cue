//! File: snippets/tests/policy.rs
//! Purpose: Protect that saving and typing agree: the page's rules decide both whether two saved triggers clash and which one fires while typing, and a `{snippet:...}` include finds what the saved list says, over a real database and the real matcher and index.
//! Contents: scenario tests that save drafts under a chosen policy, build the matcher from what is stored and type the triggers; tests of includes answered from the expander's index.

use std::collections::HashMap;

use super::helpers::{draft, open_db, save, typed, with_mode};
use crate::platform::Typed;
use crate::snippets::expand::lookup::names;
use crate::snippets::expand::matcher::{Matcher, Tracker};
use crate::snippets::expand::Index;
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
    let immediate = with_mode(Mode::Immediate);
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
    let delimiter = with_mode(Mode::Delimiter);
    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Short", &["@@"], "x"), &delimiter).expect("first");
    store::save(&conn, None, &draft("Long", &["@@p"], "y"), &delimiter).expect("the longer one can still be typed");
    let long = store::list(&conn).expect("list").into_iter().find(|s| s.name == "Long").expect("long").id;
    assert_eq!(first_fired(&conn, &delimiter, "@@p "), Some(long));

    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Short", &["@@"], "x"), &Policy::default()).expect("first");
    assert!(store::save(&conn, None, &draft("Long", &["@@p"], "y"), &Policy::default()).is_err());
}

/// Includes are answered from the index of the saved library: by name or trigger in any case, never a disabled snippet, and of two snippets sharing a name the one used most (the first in `store::list`) wins.
#[test]
fn includes_resolve_from_the_index_in_the_saved_order() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Signature", &["bestsig"], "Best")).expect("signature");
    let mut off = draft("Hidden", &["hid"], "secret");
    off.enabled = false;
    save(&conn, None, &off).expect("hidden");
    save(&conn, None, &draft("Dup", &["dupone"], "first")).expect("first dup");
    save(&conn, None, &draft("Dup", &["duptwo"], "second")).expect("second dup");
    let second = store::list(&conn).expect("list").into_iter().find(|s| s.body == "second").expect("second").id;
    store::bump(&conn, second).expect("bump");

    let library = store::list(&conn).expect("list");
    let id = |body: &str| library.iter().find(|s| s.body == body).expect("saved").id;
    let expected: HashMap<String, Vec<i64>> = [
        ("signature", vec!["Best"]),
        ("bestsig", vec!["Best"]),
        ("dup", vec!["second", "first"]),
        ("duptwo", vec!["second"]),
        ("dupone", vec!["first"]),
    ]
    .into_iter()
    .map(|(name, bodies)| (name.to_string(), bodies.into_iter().map(id).collect()))
    .collect();
    assert_eq!(names(&library), expected);
    assert_eq!(
        typed(&conn, "{snippet:SIGNATURE}|{snippet:BestSig}|{snippet:Hidden}|{snippet:hid}|{snippet:dup}"),
        "Best|Best|{snippet:Hidden}|{snippet:hid}|second"
    );
}

/// A text too big to keep in memory (over 4096 bytes) is still included in full: it is read from the file when an include asks for it.
#[test]
fn a_big_included_text_is_read_from_the_file() {
    let (_dir, conn) = open_db();
    let big = "terms;".repeat(1000);
    save(&conn, None, &draft("Terms", &["terms"], &big)).expect("terms");
    assert_eq!(typed(&conn, "<{snippet:terms}>"), format!("<{big}>"));
}

/// An index kept while snippets are used follows the uses for a shared name, as the saved list would order them: the one used most wins, without reading the library again.
#[test]
fn a_kept_index_follows_the_uses_for_a_shared_name() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Dup", &["dupone"], "first")).expect("first dup");
    save(&conn, None, &draft("Dup", &["duptwo"], "second")).expect("second dup");
    let id = |body: &str| store::list(&conn).expect("list").into_iter().find(|s| s.body == body).expect("saved").id;
    let (first, second) = (id("first"), id("second"));
    store::bump(&conn, first).expect("bump first");
    let index = Index::read(&conn).expect("index");
    let include = || index.include("dup", &|_| None);
    assert_eq!(include().as_deref(), Some("first"));
    index.counted(second);
    assert_eq!(include().as_deref(), Some("first"), "a tie keeps the order the index was read in");
    index.counted(second);
    assert_eq!(include().as_deref(), Some("second"));
    index.counted(9999);
    assert_eq!(include().as_deref(), Some("second"), "an unknown id changes nothing");
}

/// Snippets with different names sharing an include name keep the saved order at a tie of uses: the name decides (`ORDER BY uses DESC, name COLLATE NOCASE`), so a kept index includes what a fresh read of the library includes, before and after the uses meet.
#[test]
fn a_tie_of_uses_on_a_shared_name_follows_the_names_as_a_fresh_read_does() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Zeta", &[":zeta"], "from zeta")).expect("zeta");
    save(&conn, None, &draft("alpha", &["Zeta"], "from alpha")).expect("alpha");
    let id = |name: &str| store::list(&conn).expect("list").into_iter().find(|s| s.name == name).expect(name).id;
    let (zeta, alpha) = (id("Zeta"), id("alpha"));
    for used in [zeta, zeta, alpha] {
        store::bump(&conn, used).expect("bump");
    }
    let kept = Index::read(&conn).expect("index");
    let fresh = || Index::read(&conn).expect("index").include("zeta", &|_| None);
    assert_eq!(kept.include("zeta", &|_| None).as_deref(), Some("from zeta"));
    assert_eq!(fresh().as_deref(), Some("from zeta"));
    store::bump(&conn, alpha).expect("bump alpha");
    kept.counted(alpha);
    assert_eq!(fresh().as_deref(), Some("from alpha"), "at a tie a fresh read puts alpha first");
    assert_eq!(kept.include("zeta", &|_| None).as_deref(), Some("from alpha"), "the kept index agrees");
    store::bump(&conn, zeta).expect("bump zeta");
    kept.counted(zeta);
    assert_eq!(fresh().as_deref(), Some("from zeta"));
    assert_eq!(kept.include("zeta", &|_| None).as_deref(), Some("from zeta"));
}
