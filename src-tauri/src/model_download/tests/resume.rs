//! File: model_download/tests/resume.rs
//! Purpose: Protect the pure rules that vet a server's answer to a resume request.
//! Contents: tests of `resume::{strong, identity, check_range, retry_after}`.

use std::time::Duration;

use crate::model_download::resume::{check_range, identity, retry_after, strong};

/// Only a quoted, non-weak ETag may guard a resume.
#[test]
fn only_strong_etags_are_validators() {
    assert!(strong("\"abc\""));
    assert!(!strong("W/\"abc\""));
    assert!(!strong("abc"));
    assert!(!strong("\""));
}

/// An answer in a content coding cannot match the pinned hash and is refused; identity and silence are fine.
#[test]
fn encoded_bodies_are_not_identity() {
    assert!(identity(None) && identity(Some("identity")) && identity(Some(" Identity ")));
    assert!(!identity(Some("gzip")) && !identity(Some("br")));
}

/// The Content-Range must start at the resume byte, end at the last byte of the file and agree on the total.
#[test]
fn content_range_must_cover_exactly_the_rest_of_the_file() {
    assert!(check_range(Some("bytes 100-999/1000"), 100, 1000).is_ok());
    assert!(check_range(Some("bytes 100-999/*"), 100, 1000).is_ok());
    assert!(check_range(Some("bytes 50-999/1000"), 100, 1000).is_err());
    assert!(check_range(Some("bytes 100-899/1000"), 100, 1000).is_err());
    assert!(check_range(Some("bytes 100-999/2000"), 100, 1000).is_err());
    assert!(check_range(Some("items 100-999/1000"), 100, 1000).is_err());
    assert!(check_range(None, 100, 1000).is_err());
}

/// Retry-After in seconds is honoured up to two minutes; dates and junk are ignored.
#[test]
fn retry_after_reads_seconds_and_caps_them() {
    assert_eq!(retry_after(Some("7")), Some(Duration::from_secs(7)));
    assert_eq!(retry_after(Some("99999")), Some(Duration::from_secs(120)));
    assert_eq!(retry_after(Some("Wed, 21 Oct 2026 07:28:00 GMT")), None);
    assert_eq!(retry_after(None), None);
}
