//! File: snippets/tests/imports.rs
//! Purpose: Protect snippet files: Cue's own export reads back unchanged, a file from elsewhere is untrusted (no environment reads, no newer formats, bad entries counted), and a bulk import skips duplicates, clashes and invalid entries without leaving stray folders.
//! Contents: tests of `transfer::export` / `transfer::parse` and `bulk::add_all`.

use super::helpers::{draft, open_db, save};
use crate::snippets::bulk::{add_all, MAX_FOLDERS};
use crate::snippets::model::{Draft, FolderDraft, Insert};
use crate::snippets::policy::Policy;
use crate::snippets::{folders, store, transfer};

/// Export then import gives back the same snippets with every stored option and the name of their folder.
#[test]
fn an_exported_library_reads_back_with_every_option() {
    let (_dir, conn) = open_db();
    let work = folders::save_folder(
        &conn,
        None,
        &FolderDraft { name: "Work".into(), enabled: true, only_apps: vec![], skip_apps: vec![] },
        &Policy::default(),
    )
    .expect("folder");
    let mut signature = draft("Signature", &[":sig", ";sig"], "Best,\n{cursor}");
    signature.folder_id = Some(work);
    signature.insert = Insert::Rich;
    signature.only_apps = vec!["Slack\tslack".into()];
    let mut off = draft("Disabled", &["dis"], "x");
    off.enabled = false;
    save(&conn, None, &signature).expect("signature");
    save(&conn, None, &off).expect("off");

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

/// The insert names Cue stores read as themselves; any other value (the removed `auto` and `slow`, a typo) refuses that entry and only that one.
#[test]
fn an_unknown_insert_value_refuses_the_entry() {
    for (value, kept) in [("paste", Some(Insert::Paste)), ("rich", Some(Insert::Rich)), ("auto", None), ("slow", None), ("typed", None)] {
        let file = format!(
            r#"{{"version":3,"snippets":[{{"name":"Mail","triggers":[";em"],"body":"a@b.com","insert":"{value}"}},{{"name":"Ok","triggers":[";ok"],"body":"fine"}}]}}"#
        );
        let (parsed, refused) = transfer::parse(&file).expect("file reads");
        let mail = parsed.iter().find(|(d, _)| d.name == "Mail").map(|(d, _)| d.insert);
        assert_eq!((mail, refused), (kept, usize::from(kept.is_none())), "{value}");
    }
}

/// A file written before the per-snippet options were removed still imports: their keys are ignored and the rest reads as written.
#[test]
fn a_file_with_the_removed_per_snippet_keys_still_imports() {
    let file = r#"{"version":3,"snippets":[{"name":"Sig","triggers":[":sig"],"body":"Best","folder":"Work","mode":"immediate","match_case":true,
        "enabled":false,"insert":"rich","ending":"!","inside_word":true,"propagate_case":false,"only_apps":["slack"],"skip_apps":["code"]}]}"#;
    let (parsed, refused) = transfer::parse(file).expect("old file");
    assert_eq!(refused, 0);
    let expected = Draft {
        enabled: false,
        insert: Insert::Rich,
        only_apps: vec!["slack".into()],
        skip_apps: vec!["code".into()],
        ..draft("Sig", &[":sig"], "Best")
    };
    assert_eq!(parsed, [(expected, "Work".to_string())]);
}

/// The export writes exactly the fields a snippet has, none of the removed per-snippet options.
#[test]
fn the_export_holds_only_the_snippet_fields() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Sig", &[":sig"], "Best")).expect("save");
    let text = transfer::export(&store::list(&conn).expect("list"), &[]);
    let file: serde_json::Value = serde_json::from_str(&text).expect("json");
    let mut keys: Vec<&String> = file["snippets"][0].as_object().expect("entry").keys().collect();
    keys.sort();
    assert_eq!(keys, ["body", "enabled", "folder", "insert", "name", "only_apps", "skip_apps", "triggers"]);
}

/// A bulk import adds only what is new, valid and clash-free; an invalid entry does not leave its folder behind, and duplicates are skipped.
#[test]
fn bulk_import_skips_duplicates_clashes_and_invalid_entries_without_stray_folders() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Sig", &[":sig"], "Best")).expect("existing");
    let entries = vec![
        (draft("Sig", &[":sig2"], "Best"), String::new()),
        (draft("Clash", &[":sigx"], "other"), String::new()),
        (draft("Empty", &[":empty"], "   "), "Ghost".to_string()),
        (draft("Fresh", &[":fresh"], "new text"), "Imported".to_string()),
    ];
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok(1));
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
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok(MAX_FOLDERS));
    assert_eq!(folders::folders(&conn).expect("folders").len(), MAX_FOLDERS);
}

/// An import entry whose new folder name is too long is skipped whole, folder and triggers: a valid entry with the same trigger after it is added.
#[test]
fn an_entry_with_an_invalid_folder_name_is_skipped_without_taking_its_trigger() {
    let (_dir, conn) = open_db();
    let entries = vec![(draft("Bad", &[":same"], "one"), "x".repeat(81)), (draft("Good", &[":same"], "two"), "Fine".to_string())];
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok(1));
    let names: Vec<String> = folders::folders(&conn).expect("folders").into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["Fine"]);
    assert_eq!(store::list(&conn).expect("list")[0].name, "Good");
}
