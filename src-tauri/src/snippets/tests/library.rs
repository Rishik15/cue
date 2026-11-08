//! File: snippets/tests/library.rs
//! Purpose: Protect the saved library: snippets list by use, triggers that cannot coexist are refused with a reason, a stale edit never invents a snippet, and folders switch groups on and off.
//! Contents: scenario tests over a real SQLite file.

use super::helpers::{draft, open_db};
use crate::snippets::model::FolderDraft;
use crate::snippets::{folders, store};

/// Typing `:si` would fire before `:sig` can be typed, so a prefix clash is refused like an equal trigger; letter triggers that wait for a space may coexist, and disabled snippets never clash.
#[test]
fn clashing_triggers_are_refused_with_a_reason_and_harmless_ones_are_not() {
    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Signature", &[":sig"], "Best,\nRishik")).expect("first");
    for clash in [":SIG", ":si", ":sigx"] {
        let Err(message) = store::save(&conn, None, &draft("Other", &[clash], "x")) else { panic!("{clash} was accepted") };
        assert!(message.contains("clashes with :sig"), "{message}");
    }
    assert!(store::save(&conn, None, &draft("Plain word", &["sig"], "x")).is_ok());
    let mut off = draft("Off", &[":sig"], "x");
    off.enabled = false;
    assert!(store::save(&conn, None, &off).is_ok());
    assert_eq!(store::list(&conn).expect("list").len(), 3);
}

/// The snippets used most come first, and a replace keeps the snippet's identity and may keep its own trigger.
#[test]
fn most_used_snippets_come_first_and_an_edit_may_keep_its_trigger() {
    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Signature", &[":sig"], "Best")).expect("sig");
    store::save(&conn, None, &draft("Address", &[":addr"], "12 Main St")).expect("addr");
    let address = store::list(&conn).expect("list").into_iter().find(|s| s.name == "Address").expect("address");
    store::bump(&conn, address.id).expect("bump");
    assert_eq!(store::list(&conn).expect("list")[0].name, "Address");
    store::save(&conn, Some(address.id), &draft("Address", &[":addr"], "14 Main St")).expect("edit keeps its own trigger");
    let edited = store::get(&conn, address.id).expect("get").expect("exists");
    assert_eq!((edited.body.as_str(), edited.uses), ("14 Main St", 1));
}

/// Saving over a snippet that was deleted meanwhile (a stale page) is an error and never silently adds a new row.
#[test]
fn saving_an_unknown_id_fails_instead_of_creating_a_snippet() {
    let (_dir, conn) = open_db();
    store::save(&conn, None, &draft("Signature", &[":sig"], "Best")).expect("sig");
    let Err(message) = store::save(&conn, Some(9999), &draft("Ghost", &[":ghost"], "boo")) else { panic!("stale save was accepted") };
    assert_eq!(message, "That snippet no longer exists. Reopen the Snippets page.");
    assert_eq!(store::list(&conn).expect("list").len(), 1);
    let id = store::list(&conn).expect("list")[0].id;
    store::remove(&conn, id).expect("remove");
    assert!(store::get(&conn, id).expect("get").is_none());
}

/// A folder carries its programs and its switch to every snippet in it: turned off, its triggers vanish; deleted, its snippets stay and work everywhere.
#[test]
fn a_folder_switches_its_snippets_and_deleting_it_keeps_them() {
    let (_dir, conn) = open_db();
    let work = FolderDraft { name: "Work".into(), enabled: true, only_apps: vec!["Slack\tslack".into()], skip_apps: vec![] };
    let id = folders::save_folder(&conn, None, &work).expect("folder");
    let mut standup = draft("Standup", &[":standup"], "Yesterday, today, blockers");
    standup.folder_id = Some(id);
    store::save(&conn, None, &standup).expect("snippet");
    let live = store::effective_all(&conn).expect("live");
    assert_eq!((live.len(), live[0].only_apps.as_slice()), (1, ["Slack\tslack".to_string()].as_slice()));

    folders::save_folder(&conn, Some(id), &FolderDraft { enabled: false, ..work.clone() }).expect("switch off");
    assert!(store::effective_all(&conn).expect("live").is_empty());

    let Err(message) = folders::save_folder(&conn, None, &FolderDraft { name: "work".into(), ..work }) else { panic!("duplicate folder") };
    assert_eq!(message, "A folder with that name exists.");

    folders::remove_folder(&conn, id).expect("delete folder");
    let kept = store::list(&conn).expect("list");
    assert_eq!((kept.len(), kept[0].folder_id), (1, None));
    assert_eq!(store::effective_all(&conn).expect("live").len(), 1);
}
