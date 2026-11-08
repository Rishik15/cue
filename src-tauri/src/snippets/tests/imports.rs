//! File: snippets/tests/imports.rs
//! Purpose: Protect snippet files: Cue's own export reads back unchanged, a file from elsewhere is untrusted (no environment reads, no newer formats, bad entries counted), and a bulk import skips duplicates, clashes and invalid entries without leaving stray folders.
//! Contents: tests of `transfer::export` / `transfer::parse` and `bulk::add_all`.

use super::helpers::{draft, open_db};
use crate::snippets::bulk::{add_all, MAX_FOLDERS};
use crate::snippets::model::{Draft, FolderDraft, Insert, Mode};
use crate::snippets::{folders, store, transfer};

/// Export then import gives back the same snippets with every option and the name of their folder.
#[test]
fn an_exported_library_reads_back_with_every_option() {
    let (_dir, conn) = open_db();
    let work = folders::save_folder(&conn, None, &FolderDraft { name: "Work".into(), enabled: true, only_apps: vec![], skip_apps: vec![] })
        .expect("folder");
    let mut signature = draft("Signature", &[":sig", ";sig"], "Best,\n{cursor}");
    signature.folder_id = Some(work);
    (signature.mode, signature.insert, signature.inside_word) = (Mode::Immediate, Insert::Rich, true);
    signature.only_apps = vec!["Slack\tslack".into()];
    signature.ending = ".,".into();
    let mut off = draft("Disabled", &["dis"], "x");
    (off.enabled, off.match_case) = (false, true);
    store::save(&conn, None, &signature).expect("signature");
    store::save(&conn, None, &off).expect("off");

    let text = transfer::export(&store::list(&conn).expect("list"), &folders::folders(&conn).expect("folders"));
    let (parsed, refused) = transfer::parse(&text).expect("reads back");
    assert_eq!(refused, 0);
    let find = |name: &str| parsed.iter().find(|(d, _)| d.name == name).expect(name);
    assert_eq!(find("Signature"), &(Draft { folder_id: None, ..signature }, "Work".to_string()));
    assert_eq!(find("Disabled"), &(Draft { folder_id: None, ..off }, String::new()));
}

/// A file from elsewhere cannot read the environment into typed text: `{env:...}` becomes literal text, other placeholders stay live.
#[test]
fn imported_env_placeholders_become_literal_text() {
    let text =
        r#"{"version":3,"snippets":[{"name":"Token","triggers":[":tok"],"body":"Token {env:GITHUB_TOKEN} {ENV : HOME} on {date}"}]}"#;
    let (parsed, refused) = transfer::parse(text).expect("parses");
    assert_eq!(refused, 0);
    assert_eq!(parsed[0].0.body, "Token {{env:GITHUB_TOKEN}} {{ENV : HOME}} on {date}");
}

/// Newer files, non-files and invalid entries: a newer format is refused whole, garbage is refused with a plain message, and bad entries are counted, not fatal.
#[test]
fn newer_or_broken_files_are_refused_and_bad_entries_are_counted() {
    let newer = r#"{"version":99,"snippets":[]}"#;
    assert_eq!(transfer::parse(newer).err().as_deref(), Some("This file comes from a newer version of Cue."));
    assert_eq!(transfer::parse("not json at all").err().as_deref(), Some("This is not a snippets file Cue can read."));
    let long_trigger = "x".repeat(41);
    let mixed = format!(
        "\u{feff}{{\"version\":3,\"snippets\":[{{\"name\":\"Good\",\"triggers\":[\":ok\"],\"body\":\"fine\"}},{{\"triggers\":[\":noname\"],\"body\":\"b\"}},{{\"name\":\"Long\",\"triggers\":[\"{long_trigger}\"],\"body\":\"b\"}}]}}"
    );
    let (parsed, refused) = transfer::parse(&mixed).expect("a byte order mark is skipped");
    assert_eq!((parsed.len(), refused), (1, 2));
}

/// Older formats still read: Raycast's bare list, and version 1 or 2 files with a single keyword and a paste flag.
#[test]
fn older_and_raycast_files_still_read() {
    let raycast = r#"[{"name":"Sig","text":"Best","keyword":":sig"}]"#;
    let (parsed, _) = transfer::parse(raycast).expect("raycast");
    assert_eq!((parsed[0].0.triggers.as_slice(), parsed[0].0.body.as_str()), ([":sig".to_string()].as_slice(), "Best"));
    let old = r#"{"version":1,"snippets":[{"name":"Mail","keyword":";em","text":"a@b.com","paste":true}]}"#;
    let (parsed, _) = transfer::parse(old).expect("old");
    assert_eq!((parsed[0].0.insert, parsed[0].0.triggers[0].as_str()), (Insert::Paste, ";em"));
}

/// A bulk import adds only what is new, valid and clash-free; an invalid entry does not leave its folder behind, and duplicates are skipped.
#[test]
fn bulk_import_skips_duplicates_clashes_and_invalid_entries_without_stray_folders() {
    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Sig", &[":sig"], "Best")).expect("existing");
    let entries = vec![
        (draft("Sig", &[":sig2"], "Best"), String::new()),
        (draft("Clash", &[":sigx"], "other"), String::new()),
        (draft("Empty", &[":empty"], "   "), "Ghost".to_string()),
        (draft("Fresh", &[":fresh"], "new text"), "Imported".to_string()),
    ];
    assert_eq!(add_all(&conn, &entries), Ok(1));
    let names: Vec<String> = folders::folders(&conn).expect("folders").into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["Imported"]);
    let fresh = store::list(&conn).expect("list").into_iter().find(|s| s.name == "Fresh").expect("fresh");
    assert!(fresh.folder_id.is_some());
}

/// A file naming a different folder for every entry cannot bury the page: folders stop at the cap and the rest of the entries are skipped.
#[test]
fn bulk_import_stops_making_folders_at_the_cap() {
    let (_dir, conn) = open_db();
    let entries: Vec<(Draft, String)> = (0..=MAX_FOLDERS)
        .map(|i| (draft(&format!("S{i}"), &[&format!(":q{i:03}")], &format!("body {i}")), format!("folder {i}")))
        .collect();
    assert_eq!(add_all(&conn, &entries), Ok(MAX_FOLDERS));
    assert_eq!(folders::folders(&conn).expect("folders").len(), MAX_FOLDERS);
}
