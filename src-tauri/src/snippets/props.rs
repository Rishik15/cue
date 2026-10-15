//! Purpose: Property tests for the snippet file: anything that passes the limits survives an export and an import unchanged, and no file content makes the reader panic.
use proptest::prelude::*;

use super::check::{check, clean};
use super::model::{Draft, Insert, Mode, Snippet};
use super::transfer::{export, parse};

proptest! {
    #[test]
    fn exported_snippets_read_back_the_same(entries in proptest::collection::vec(("[a-zA-Z0-9 ]{1,40}", "[a-z:]{0,10}", ".{1,200}", any::<bool>()), 0..8)) {
        let drafts: Vec<Draft> = entries
            .into_iter()
            .map(|(name, trigger, body, match_case)| Draft { name, triggers: vec![trigger], body, match_case, skip_apps: vec!["slack".into()], ..Draft::default() })
            .filter(|d| check(&clean(d)).is_ok())
            .collect();
        let snippets: Vec<Snippet> = drafts
            .iter()
            .enumerate()
            .map(|(i, d)| Snippet {
                id: i as i64,
                name: d.name.clone(),
                triggers: d.triggers.clone(),
                body: d.body.clone(),
                uses: 0,
                last_used: 0,
                folder_id: None,
                mode: Mode::Auto,
                match_case: d.match_case,
                enabled: d.enabled,
                insert: Insert::Auto,
                ending: d.ending.clone(),
                inside_word: d.inside_word,
                propagate_case: d.propagate_case,
                only_apps: d.only_apps.clone(),
                skip_apps: d.skip_apps.clone(),
            })
            .collect();
        let (back, refused) = parse(&export(&snippets, &[])).unwrap();
        prop_assert_eq!(refused, 0);
        prop_assert_eq!(back.into_iter().map(|(d, _)| d).collect::<Vec<_>>(), drafts);
    }

    #[test]
    fn any_file_content_is_refused_or_read_without_panicking(text in ".{0,500}") {
        let _ = parse(&text);
    }
}
