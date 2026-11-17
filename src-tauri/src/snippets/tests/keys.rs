//! File: snippets/tests/keys.rs
//! Purpose: Protect the page's settings: the settings door accepts exactly what each key means, and the page spells every key the expander reads (the two lists are one contract).
//! Contents: scenario tests of `keys::check` and a check of `src/snippets.ts` against the Rust keys.

use serde_json::json;

use crate::snippets::keys::{self, check};

/// The page's source, where `SETTING` lists the keys it reads and writes.
const PAGE: &str = include_str!("../../../../src/snippets.ts");

/// Each kind of key takes its own kind of value and range; a key of another feature is not decided here.
#[test]
fn the_settings_door_takes_what_each_key_means() {
    assert_eq!(check(keys::MODE, &json!("delimiter")), Some(true));
    assert_eq!(check(keys::MODE, &json!("slow")), Some(false));
    assert_eq!(check(keys::UNDO, &json!(true)), Some(true));
    assert_eq!(check(keys::UNDO, &json!("yes")), Some(false));
    assert_eq!(check(keys::DELAY_MS, &json!(600)), Some(true));
    assert_eq!(check(keys::DELAY_MS, &json!(1001)), Some(false));
    assert_eq!(check(keys::RESPONSE_MS, &json!(-1)), Some(false));
    assert_eq!(check(keys::ENDING, &json!(" .,")), Some(true));
    assert_eq!(check(keys::ENDING, &json!("a\nb")), Some(false));
    assert_eq!(check(keys::ENDING, &json!("x".repeat(keys::MAX_ENDING + 1))), Some(false));
    assert_eq!(check("snippets_insert", &json!("paste")), None, "a removed key stays unknown");
    assert_eq!(check("notes_paper", &json!("Plain")), None);
}

/// Every key the expander reads is spelled in the page's `SETTING` list, so a renamed key cannot leave the page writing to a key nothing reads.
#[test]
fn the_page_spells_every_key_rust_reads() {
    for key in [
        keys::EXPAND,
        keys::SKIP_APPS,
        keys::UNDO,
        keys::CONFIRM_DELETE,
        keys::SOUND,
        keys::MODE,
        keys::MATCH_CASE,
        keys::FOLLOW_CASE,
        keys::INSIDE_WORDS,
        keys::ENDING,
        keys::DELAY_MS,
        keys::RESPONSE_MS,
    ] {
        assert!(PAGE.contains(&format!("\"{key}\"")), "{key} is not in src/snippets.ts");
    }
}
