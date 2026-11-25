//! File: snippets/tests/library.rs
//! Purpose: Protect the saved library: snippets list by use, triggers that cannot coexist are refused with a reason on every kind of write, a stale edit never invents a snippet, and folders switch groups on and off.
//! Contents: scenario tests over a real SQLite file.

use std::sync::{Arc, Barrier};

use super::helpers::{draft, open_db, save};
use crate::snippets::bulk::add_all;
use crate::snippets::check::MAX_SNIPPETS;
use crate::snippets::model::{Draft, FolderDraft, Mode};
use crate::snippets::policy::Policy;
use crate::snippets::{folders, store};

/// Typing `:si` would fire before `:sig` can be typed, so a prefix clash is refused like an equal trigger; letter triggers that wait for a space may coexist, and disabled snippets never clash.
#[test]
fn clashing_triggers_are_refused_with_a_reason_and_harmless_ones_are_not() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Signature", &[":sig"], "Best,\nRishik")).expect("first");
    for clash in [":SIG", ":si", ":sigx"] {
        let Err(message) = save(&conn, None, &draft("Other", &[clash], "x")) else { panic!("{clash} was accepted") };
        assert!(message.contains("clashes with :sig"), "{message}");
    }
    assert!(save(&conn, None, &draft("Plain word", &["sig"], "x")).is_ok());
    let mut off = draft("Off", &[":sig"], "x");
    off.enabled = false;
    assert!(save(&conn, None, &off).is_ok());
    assert_eq!(store::list(&conn).expect("list").len(), 3);
}

/// The snippets used most come first, and a replace keeps the snippet's identity and may keep its own trigger.
#[test]
fn most_used_snippets_come_first_and_an_edit_may_keep_its_trigger() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Signature", &[":sig"], "Best")).expect("sig");
    save(&conn, None, &draft("Address", &[":addr"], "12 Main St")).expect("addr");
    let address = store::list(&conn).expect("list").into_iter().find(|s| s.name == "Address").expect("address");
    store::bump(&conn, address.id).expect("bump");
    assert_eq!(store::list(&conn).expect("list")[0].name, "Address");
    save(&conn, Some(address.id), &draft("Address", &[":addr"], "14 Main St")).expect("edit keeps its own trigger");
    let edited = store::get(&conn, address.id).expect("get").expect("exists");
    assert_eq!((edited.body.as_str(), edited.uses), ("14 Main St", 1));
}

/// Saving over a snippet that was deleted meanwhile (a stale page) is an error and never silently adds a new row.
#[test]
fn saving_an_unknown_id_fails_instead_of_creating_a_snippet() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("Signature", &[":sig"], "Best")).expect("sig");
    let Err(message) = save(&conn, Some(9999), &draft("Ghost", &[":ghost"], "boo")) else { panic!("stale save was accepted") };
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
    let id = folders::save_folder(&conn, None, &work, &Policy::default()).expect("folder");
    let mut standup = draft("Standup", &[":standup"], "Yesterday, today, blockers");
    standup.folder_id = Some(id);
    save(&conn, None, &standup).expect("snippet");
    let live = store::effective_all(&conn, &Policy::default()).expect("live");
    assert_eq!((live.len(), live[0].only_apps.as_slice()), (1, ["Slack\tslack".to_string()].as_slice()));

    folders::save_folder(&conn, Some(id), &FolderDraft { enabled: false, ..work.clone() }, &Policy::default()).expect("switch off");
    assert!(store::effective_all(&conn, &Policy::default()).expect("live").is_empty());

    let Err(message) = folders::save_folder(&conn, None, &FolderDraft { name: "work".into(), ..work }, &Policy::default()) else {
        panic!("duplicate folder")
    };
    assert_eq!(message, "A folder with that name exists.");

    folders::remove_folder(&conn, id).expect("delete folder");
    let kept = store::list(&conn).expect("list");
    assert_eq!((kept.len(), kept[0].folder_id), (1, None));
    assert_eq!(store::effective_all(&conn, &Policy::default()).expect("live").len(), 1);
}

/// Saves each draft on its own connection to the one file, all released together. Returns the results.
fn save_in_parallel(dir: &std::path::Path, drafts: Vec<Draft>) -> Vec<Result<(), String>> {
    let gate = Arc::new(Barrier::new(drafts.len()));
    let workers: Vec<_> = drafts
        .into_iter()
        .map(|d| {
            let (conn, gate) = (store::open(&dir.join("snippets.db")).expect("open"), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                save(&conn, None, &d)
            })
        })
        .collect();
    workers.into_iter().map(|w| w.join().expect("thread")).collect()
}

/// Two saves on two connections cannot both pass the check and both commit: the clash and the snippet limit hold across writers.
#[test]
fn two_writers_cannot_both_commit_a_clash() {
    let (dir, conn) = open_db();
    let done = save_in_parallel(dir.path(), vec![draft("A", &[":abc"], "x"), draft("B", &[":abc"], "y")]);
    assert_eq!(done.iter().filter(|r| r.is_ok()).count(), 1, "{done:?}");
    assert!(done.iter().any(|r| r.as_ref().is_err_and(|m| m.contains("clashes with :abc"))), "{done:?}");
    assert_eq!(store::list(&conn).expect("list").len(), 1);

    let full: Vec<(Draft, String)> = (0..MAX_SNIPPETS - 2).map(|i| (draft(&format!("n{i}"), &[], "x"), String::new())).collect();
    assert_eq!(add_all(&conn, &full, &Policy::default()), Ok(MAX_SNIPPETS - 2));
    let done = save_in_parallel(dir.path(), vec![draft("C", &[], "x"), draft("D", &[], "y")]);
    assert_eq!(done.iter().filter(|r| r.is_ok()).count(), 1, "{done:?}");
    assert_eq!(store::list(&conn).expect("list").len(), MAX_SNIPPETS);
}

/// One snippet's own triggers obey the same rule as two snippets': `:a` would fire before `:ab` is typed. Triggers that wait for a delimiter may share a start.
#[test]
fn a_snippet_cannot_clash_with_itself() {
    let (_dir, conn) = open_db();
    let Err(message) = save(&conn, None, &draft("Self", &[":a", ":ab"], "x")) else { panic!(":a and :ab were accepted") };
    assert!(message.contains("clashes with :a"), "{message}");
    assert!(save(&conn, None, &draft("Self", &[":a", "b"], "x")).is_ok());
    assert!(save(&conn, None, &draft("Words", &["sig", "sigx"], "x")).is_ok());
    assert_eq!(store::list(&conn).expect("list").len(), 2);
}

/// Switching a folder on turns its triggers on: a clash with another snippet refuses it and the folder stays off. A rename of a folder that stays off is not checked.
#[test]
fn enabling_a_folder_refuses_a_clash() {
    let (_dir, conn) = open_db();
    let policy = Policy::default();
    let off = FolderDraft { name: "Off".into(), enabled: false, only_apps: vec![], skip_apps: vec![] };
    let id = folders::save_folder(&conn, None, &off, &policy).expect("folder");
    let mut inside = draft("Inside", &[":sig"], "x");
    inside.folder_id = Some(id);
    save(&conn, None, &inside).expect("snippet in the off folder");
    save(&conn, None, &draft("Outside", &[":si"], "y")).expect("the shorter trigger is free while the folder is off");

    let Err(message) = folders::save_folder(&conn, Some(id), &FolderDraft { enabled: true, ..off.clone() }, &policy) else {
        panic!("enabled with a clash")
    };
    assert!(message.contains(":sig") && message.contains(":si"), "{message}");
    assert!(!folders::folders(&conn).expect("folders")[0].enabled);
    folders::save_folder(&conn, Some(id), &FolderDraft { name: "Renamed".into(), ..off.clone() }, &policy).expect("rename while off");

    let other = store::list(&conn).expect("list").into_iter().find(|s| s.name == "Outside").expect("outside");
    store::remove(&conn, other.id).expect("remove");
    folders::save_folder(&conn, Some(id), &FolderDraft { name: "Renamed".into(), enabled: true, ..off }, &policy)
        .expect("enabled without a clash");
}

/// `sig` and `sigx` both wait for a delimiter under Auto and Delimiter; made immediate they clash, so the change of timing is refused.
#[test]
fn a_timing_change_that_creates_a_clash_is_refused() {
    let (_dir, conn) = open_db();
    save(&conn, None, &draft("One", &["sig"], "x")).expect("sig");
    save(&conn, None, &draft("Two", &["sigx"], "y")).expect("sigx");
    let Err(message) = store::check_library(&conn, &Policy::with_mode(Mode::Immediate)) else { panic!("Immediate was accepted") };
    assert!(message.contains("sig") && message.contains("clashes"), "{message}");
    assert!(store::check_library(&conn, &Policy::with_mode(Mode::Delimiter)).is_ok());
    assert!(store::check_library(&conn, &Policy::with_mode(Mode::Auto)).is_ok());
}
