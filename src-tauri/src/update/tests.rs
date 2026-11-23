//! File: update/tests.rs
//! Purpose: Protect the update path against a hostile or broken release: only strictly newer plain versions count, the installer comes from one address, and a tampered or unsigned file is never accepted.
//! Contents: tests of `newer`, `installer_url`, `verify_file`, and the pure `settle` / `apply` phase logic.

use std::path::PathBuf;
use std::sync::Mutex;

use super::manifest::{installer_url, newer, verify_file, Manifest};
use super::phase::{apply, settle, Keep, Phase, Settled};

/// Only a strictly newer version counts, with or without a leading v, and an unreadable one never does.
#[test]
fn only_a_strictly_newer_readable_version_is_an_update() {
    assert!(newer("0.10.0", "0.9.9"));
    assert!(newer("v1.0.0", "0.99.99"));
    assert!(!newer("1.2.3", "1.2.3"));
    assert!(!newer("1.2.2", "1.2.3"));
    for bad in ["", "1.2", "1.2.x", "latest", "-1.0.0"] {
        assert!(!newer(bad, "0.0.1"), "{bad:?}");
    }
    assert!(!newer("2.0.0", "garbage"));
}

/// The installer address is built from a plain x.y.z alone, so a manifest cannot point it elsewhere.
#[test]
fn the_installer_address_is_built_only_from_a_plain_version() {
    let url = installer_url("1.4.0").expect("plain version");
    assert!(url.starts_with("https://") && url.ends_with("v1.4.0/Cue_1.4.0_x64-setup.exe"), "{url}");
    for bad in ["v1.4.0", "1.4", "1.4.0/../../evil", "1.04.0", "1.4.0-beta", "1.4.0 ", "https://evil.example/x.exe"] {
        assert_eq!(installer_url(bad), None, "{bad:?}");
    }
}

/// A file that does not match its signature, a garbage signature and a missing file are all refused with a message the user can read.
#[test]
fn a_tampered_or_unsigned_installer_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("Cue_9.9.9_x64-setup.exe");
    std::fs::write(&path, b"not the real installer").expect("write");
    let refused = "The update could not be verified, so it was not installed.";
    for signature in ["", "AAAA", "not base64 !!", "dW50cnVzdGVkIGNvbW1lbnQ6IHg="] {
        assert_eq!(verify_file(&path, signature), Err(refused.to_string()), "{signature:?}");
    }
    let missing = verify_file(&dir.path().join("gone.exe"), "AAAA").expect_err("missing file");
    assert!(missing.starts_with("Could not read the saved update"), "{missing}");
}

fn release(version: &str) -> Manifest {
    Manifest { version: version.into(), url: String::new(), signature: String::new() }
}

fn ready(version: &str) -> Phase {
    Phase::Ready { manifest: release(version), path: PathBuf::from("x.exe") }
}

/// A check never plans to delete what the current phase references, and replaces a Ready installer only by a newer one.
#[test]
fn settle_follows_the_phase_table() {
    let (old, new) = (release("1.0.0"), release("1.1.0"));
    let disk = || Some(PathBuf::from("n.exe"));
    let ready_new = Phase::Ready { manifest: new.clone(), path: PathBuf::from("n.exe") };
    let down_new = Phase::Downloading { manifest: new.clone(), percent: 0 };
    let file_new = || Keep::File("1.1.0".into());

    assert_eq!(settle(&ready("1.0.0"), None, None), Settled { next: None, keep: Keep::Untouched });
    assert_eq!(settle(&ready("1.0.0"), Some(&old), None), Settled { next: None, keep: Keep::Untouched });
    assert_eq!(settle(&ready("1.1.0"), Some(&old), None), Settled { next: None, keep: Keep::Untouched });
    assert_eq!(settle(&ready("1.0.0"), Some(&new), disk()), Settled { next: Some(ready_new), keep: file_new() });
    assert_eq!(settle(&Phase::Installing, Some(&new), None), Settled { next: None, keep: Keep::Untouched });
    assert_eq!(settle(&down_new, Some(&old), None), Settled { next: None, keep: Keep::Untouched });
    assert_eq!(settle(&Phase::Downloading { manifest: old, percent: 5 }, None, None).keep, Keep::Untouched);
    assert_eq!(settle(&Phase::None, None, None), Settled { next: None, keep: Keep::All });
    let found = settle(&Phase::None, Some(&new), None);
    assert_eq!((found.next.as_ref(), found.keep), (Some(&down_new), file_new()));
    assert!(matches!(settle(&Phase::None, Some(&new), disk()).next, Some(Phase::Ready { .. })));
}

/// An Install that claims the Ready installer while a check is on the network leaves the state Installing: the check's result changes nothing and deletes nothing.
#[test]
fn a_claim_during_a_check_is_not_overwritten() {
    let state = Mutex::new(ready("1.0.0"));
    assert!(apply(&state, |now| matches!(now, Phase::Ready { .. }).then_some(Phase::Installing)).is_some());
    let found = release("1.1.0");
    let mut keep = None;
    let changed = apply(&state, |now| {
        let settled = settle(now, Some(&found), None);
        keep = Some(settled.keep);
        settled.next
    });
    assert!(changed.is_none());
    assert_eq!(keep, Some(Keep::Untouched));
    assert_eq!(*state.lock().unwrap(), Phase::Installing);
}
