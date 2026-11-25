//! File: snippets/tests/imports.rs
//! Purpose: Protect snippet files: Cue's own export reads back unchanged, a file from elsewhere is untrusted (no environment reads directly or through an include, one format, bad entries counted), and a bulk import skips duplicates, clashes and invalid entries without leaving stray folders.
//! Contents: tests of `transfer::export` / `transfer::parse` and `bulk::add_all`; typed — an imported body rendered by the real expansion code against the saved library.

use std::collections::HashMap;

use super::helpers::{draft, open_db, save};
use crate::platform::LocalTime;
use crate::snippets::bulk::{add_all, MAX_FOLDERS};
use crate::snippets::expand::lookup::Others;
use crate::snippets::expand::template::{expand, Context};
use crate::snippets::model::{Draft, FolderDraft, Insert, Mode};
use crate::snippets::policy::Policy;
use crate::snippets::{folders, store, transfer};

/// Export then import gives back the same snippets with every option and the name of their folder.
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
    (signature.mode, signature.insert, signature.inside_word) = (Mode::Immediate, Insert::Rich, true);
    signature.only_apps = vec!["Slack\tslack".into()];
    signature.ending = ".,".into();
    let mut off = draft("Disabled", &["dis"], "x");
    (off.enabled, off.match_case) = (false, true);
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

/// The Cue reader reads exactly one format: version 1 and 2 files and a bare list are refused with a plain reason; a version 3 file reads.
#[test]
fn old_and_bare_list_files_are_refused_by_the_cue_reader() {
    let older = "This file was written by an older Cue that this version cannot read.";
    for old in [
        r#"{"version":1,"snippets":[{"name":"Mail","keyword":";em","text":"a@b.com"}]}"#,
        r#"{"version":2,"snippets":[{"name":"Mail","trigger":";em","body":"a@b.com"}]}"#,
    ] {
        assert_eq!(transfer::parse(old).err().as_deref(), Some(older), "{old}");
    }
    let bare = r#"[{"name":"Sig","text":"Best","keyword":":sig"}]"#;
    assert_eq!(transfer::parse(bare).err().as_deref(), Some("This is not a snippets file Cue can read."));
    let current = r#"{"version":3,"snippets":[{"name":"Mail","triggers":[";em"],"body":"a@b.com"}]}"#;
    let (parsed, refused) = transfer::parse(current).expect("version 3");
    assert_eq!((parsed[0].0.triggers.as_slice(), refused), ([";em".to_string()].as_slice(), 0));
}

/// The bodies stored for snippets named `names`, in that order.
fn bodies(conn: &rusqlite::Connection, names: &[&str]) -> Vec<String> {
    let all = store::list(conn).expect("list");
    names.iter().map(|n| all.iter().find(|s| s.name == *n).expect(n).body.clone()).collect()
}

/// An imported `{snippet:X}` whose saved X reads the environment (directly, through its own include, by trigger, in any case) is made literal and counted; a disabled or env-free X stays a live include.
#[test]
fn an_import_cannot_read_a_secret_through_an_include() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Secret", &[":sec"], "{env:SECRET}")).expect("secret");
    save(&conn, None, &draft("Wrapper", &[":wrap"], "via {snippet:secret}")).expect("wrapper");
    save(&conn, None, &draft("Plain", &[":plain"], "hello {date}")).expect("plain");
    let mut off = draft("Hidden", &[":hid"], "{env:OTHER}");
    off.enabled = false;
    save(&conn, None, &off).expect("hidden");
    let entries = vec![
        (draft("Direct", &[":i1"], "token {snippet:Secret}"), String::new()),
        (draft("Trigger", &[":i2"], "{ Snippet : :SEC | upper }"), String::new()),
        (draft("Through", &[":i3"], "{snippet:WRAPPER}"), String::new()),
        (draft("Live", &[":i4"], "{snippet:Plain}"), String::new()),
        (draft("Off", &[":i5"], "{snippet:Hidden}"), String::new()),
    ];
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((5, 3)));
    let got = bodies(&conn, &["Direct", "Trigger", "Through", "Live", "Off"]);
    assert_eq!(
        got,
        ["token {{snippet:Secret}}", "{{ Snippet : :SEC | upper }}", "{{snippet:WRAPPER}}", "{snippet:Plain}", "{snippet:Hidden}"]
    );
}

/// Includes between snippets of one file stay live, and Cue's own export reads back into an empty library with identical bodies.
#[test]
fn includes_between_imported_snippets_keep_working() {
    let (_dir, conn) = open_db();
    let entries = vec![(draft("A", &[":a"], "x {snippet:B}"), String::new()), (draft("B", &[":b"], "plain"), String::new())];
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((2, 0)));
    assert_eq!(bodies(&conn, &["A"]), ["x {snippet:B}"]);

    save(&conn, None, &draft("Secret", &[":sec"], "{env:SECRET}")).expect("secret");
    save(&conn, None, &draft("Uses", &[":use"], "{snippet:Secret}")).expect("uses");
    let text = transfer::export(&store::list(&conn).expect("list"), &folders::folders(&conn).expect("folders"));
    let (parsed, _) = transfer::parse(&text).expect("reads back");
    let (_empty_dir, empty) = open_db();
    assert_eq!(add_all(&empty, &parsed, &Policy::default()), Ok((4, 0)));
    assert_eq!(bodies(&empty, &["A", "B", "Uses"]), ["x {snippet:B}", "plain", "{snippet:Secret}"]);
    assert_eq!(bodies(&empty, &["Secret"]), ["{{env:SECRET}}"]);
}

/// Every removed or unknown insert value (auto, slow, a typo) imports and loads as plain paste; "rich" stays rich.
#[test]
fn removed_insert_values_read_as_plain_paste() {
    for old in ["auto", "paste", "slow", "typed"] {
        let file = format!(r#"{{"version":3,"snippets":[{{"name":"Mail","triggers":[";em"],"body":"a@b.com","insert":"{old}"}}]}}"#);
        let (parsed, _) = transfer::parse(&file).expect("old file");
        assert_eq!(parsed[0].0.insert, Insert::Paste, "{old}");
        assert_eq!(Insert::parse(old), Insert::Paste, "{old}");
    }
    assert_eq!(Insert::parse("rich"), Insert::Rich);
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
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((1, 0)));
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
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((MAX_FOLDERS, 0)));
    assert_eq!(folders::folders(&conn).expect("folders").len(), MAX_FOLDERS);
}

/// An import entry whose new folder name is too long is skipped whole, folder and triggers: a valid entry with the same trigger after it is added.
#[test]
fn an_entry_with_an_invalid_folder_name_is_skipped_without_taking_its_trigger() {
    let (_dir, conn) = open_db();
    let entries = vec![(draft("Bad", &[":same"], "one"), "x".repeat(81)), (draft("Good", &[":same"], "two"), "Fine".to_string())];
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((1, 0)));
    let names: Vec<String> = folders::folders(&conn).expect("folders").into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["Fine"]);
    assert_eq!(store::list(&conn).expect("list")[0].name, "Good");
}

/// What `body` types in Notepad through the real expansion code, with the saved snippets of `conn` as includes and every environment variable reading `TOPSECRET`. Inputs: the connection and the text. Returns the typed text.
fn typed(conn: &rusqlite::Connection, body: &str) -> String {
    let library = Others(store::list(conn).expect("list"));
    let lookup = |name: &str| library.find(name);
    let ctx = Context {
        now: LocalTime { year: 2026, month: 3, day: 5, hour: 14, minute: 7, second: 9, weekday: 4 },
        clipboard: None,
        app: "notepad".into(),
        counter: 1,
        lookup: &lookup,
        env: &|_| Some("TOPSECRET".into()),
        entropy: &|| Some(0),
        args: HashMap::new(),
        captures: HashMap::new(),
    };
    expand(body, &ctx).text
}

/// An include or `{env:}` that only forms once conditions join their branches cannot type a secret after import: each body leaks when stored as written, and none leaks as imported; a plain condition stays live.
#[test]
fn an_include_formed_by_a_condition_cannot_read_a_secret_after_import() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Secret", &[":sec"], "{env:SECRET}")).expect("secret");
    save(&conn, None, &draft("Plain", &[":plain"], "hello")).expect("plain");
    let formed = ["{snippet:Missing}{snip{if app=notepad}pet:Secret{/if}}", "{e{if app=notepad}nv:SECRET{/if}}"];
    for body in formed {
        assert!(typed(&conn, body).contains("TOPSECRET"), "the repro leaks as written: {body}");
    }
    let live = "{if app=slack}hi{else}{snippet:Plain}{/if}";
    let entries: Vec<_> = formed
        .iter()
        .chain([&live])
        .enumerate()
        .map(|(i, b)| (draft(&format!("I{i}"), &[&format!(":imp{i}")], b), String::new()))
        .collect();
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((3, 2)));
    for stored in bodies(&conn, &["I0", "I1"]) {
        assert!(!typed(&conn, &stored).contains("TOPSECRET"), "{stored}");
    }
    assert_eq!(bodies(&conn, &["I2"]), [live]);
    assert_eq!(typed(&conn, live), "hello");
}

/// A saved snippet whose hidden branch names many distinct snippets still counts as reading the environment, and one naming more than the safety walk reads fails closed: imports that include them are made literal and type no secret.
#[test]
fn an_include_past_the_discovery_budget_cannot_read_a_secret_after_import() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Secret", &[":sec"], "{env:SECRET}")).expect("secret");
    let missing: String = (0..256).map(|i| format!("{{snippet:missing{i}}}")).collect();
    save(&conn, None, &draft("Wrapper", &[":wrap"], &format!("{{if app=slack}}{missing}{{else}}{{snippet:Secret}}{{/if}}")))
        .expect("wrapper");
    assert!(typed(&conn, "{snippet:Wrapper}").contains("TOPSECRET"), "the repro leaks as written");
    let too_many: String = (0..257).map(|i| format!("{{snippet:gone{i}}}")).collect();
    save(&conn, None, &draft("Many", &[":many"], &too_many)).expect("many");
    let entries = vec![
        (draft("Imp", &[":imp"], "{snippet:Wrapper}"), String::new()),
        (draft("Unread", &[":unread"], "{snippet:Many}"), String::new()),
    ];
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((2, 2)));
    let stored = bodies(&conn, &["Imp", "Unread"]);
    assert_eq!(stored, ["{{snippet:Wrapper}}", "{{snippet:Many}}"], "past the budget counts as reading the environment");
    assert!(!typed(&conn, &stored[0]).contains("TOPSECRET"));
}

/// Names that differ only in a non-ASCII letter (the Kelvin sign) are different snippets to the lookup, so the safety walk keeps them apart: a missing `\u{212A}ey` named first does not hide the saved `Key`, directly or through an include.
#[test]
fn a_unicode_look_alike_name_cannot_hide_a_secret_include() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Key", &[":key"], "{env:SECRET}")).expect("key");
    save(&conn, None, &draft("Wrap", &[":wrap"], "{snippet:\u{212A}ey}{snippet:Key}")).expect("wrap");
    let direct = "{snippet:\u{212A}ey}{snippet:Key}";
    for body in [direct, "{snippet:Wrap}"] {
        assert!(typed(&conn, body).contains("TOPSECRET"), "the repro leaks as written: {body}");
    }
    let entries = vec![(draft("Direct", &[":i1"], direct), String::new()), (draft("Through", &[":i2"], "{snippet:Wrap}"), String::new())];
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((2, 2)));
    let stored = bodies(&conn, &["Direct", "Through"]);
    assert_eq!(stored, ["{snippet:\u{212A}ey}{{snippet:Key}}", "{{snippet:Wrap}}"]);
    for body in &stored {
        assert!(!typed(&conn, body).contains("TOPSECRET"), "{body}");
    }
}

/// Many harmless conditions stay live, directly and through an include: only the program and clipboard situations the conditions can see are tried, not every branch combination. Conditions naming more programs than the walk tries fail closed.
#[test]
fn many_harmless_conditions_import_unchanged() {
    let (_dir, conn) = open_db();
    let seven = "{if app=notepad}a{else}b{/if}".repeat(7);
    save(&conn, None, &draft("Seven", &[":seven"], &seven)).expect("seven");
    let crowded: String = (0..40).map(|i| format!("{{if app=p{i}}}x{{/if}}")).collect();
    let entries = vec![
        (draft("Direct", &[":i1"], &seven), String::new()),
        (draft("Through", &[":i2"], "{snippet:Seven}"), String::new()),
        (draft("Crowded", &[":i3"], &crowded), String::new()),
    ];
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((3, 1)));
    let stored = bodies(&conn, &["Direct", "Through", "Crowded"]);
    assert_eq!(stored[..2], [seven.clone(), "{snippet:Seven}".to_string()]);
    assert_eq!((typed(&conn, &stored[0]), typed(&conn, &stored[1])), ("aaaaaaa".to_string(), "aaaaaaa".to_string()));
    assert_ne!(stored[2], crowded, "past the budget counts as reading the environment");
}
