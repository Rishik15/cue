//! File: update/tests.rs
//! Purpose: Protect the update path against a hostile or broken release: only strictly newer plain versions count, the installer comes from one address, and a tampered or unsigned file is never accepted.
//! Contents: tests of `newer`, `installer_url` and `verify_file`.

use super::manifest::{installer_url, newer, verify_file};

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
