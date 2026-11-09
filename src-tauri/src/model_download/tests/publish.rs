//! File: model_download/tests/publish.rs
//! Purpose: Protect the publish step of every installer (speech archive, text server, dictionary pack): a cancel that wins the commit point installs nothing and cleans up, a commit that wins ignores a later cancel, and a failed unpack is not published.
//! Contents: scenario tests of `catalog_stream::publish`, `text::catalog::publish_server`, `dictionary::catalog::publish_pack` and of the commit point itself.

use std::fs;

use crate::dictionary::catalog as dictionary;
use crate::model_download::ctl::Ctl;
use crate::speech::catalog_stream::{publish, STREAMING};
use crate::text::catalog as text;

/// Writes every file of the first streaming model into `dir` with one byte each (the wrong size, so a publish that checks sizes refuses them).
fn write_files(dir: &std::path::Path) {
    STREAMING[0].files.iter().for_each(|f| fs::write(dir.join(f.name), b"x").expect("file"));
}

/// A cancelled unpack removes every file it wrote and reports the cancel.
#[test]
fn speech_publish_removes_the_files_when_cancelled() {
    let (dir, ctl) = (tempfile::tempdir().expect("dir"), Ctl::detached());
    write_files(dir.path());
    ctl.cancel_for_test();
    assert_eq!(publish(dir.path(), &STREAMING[0], true, &ctl), Err("Cancelled.".to_string()));
    assert!(STREAMING[0].files.iter().all(|f| !dir.path().join(f.name).exists()));
}

/// An unpack that exited badly or left wrong sizes is refused without taking the commit point, so a cancel after it still counts.
#[test]
fn speech_publish_refuses_incomplete_files_before_committing() {
    let (dir, ctl) = (tempfile::tempdir().expect("dir"), Ctl::detached());
    write_files(dir.path());
    assert!(publish(dir.path(), &STREAMING[0], true, &ctl).is_err());
    assert!(publish(dir.path(), &STREAMING[0], false, &ctl).is_err());
    ctl.cancel_for_test();
    assert!(ctl.is_cancelled(), "not committed, so the cancel took effect");
}

/// A cancel that wins the commit point leaves no staging folder and no server; a commit that wins installs the server even if a cancel follows.
#[test]
fn text_publish_follows_the_commit_point() {
    let root = tempfile::tempdir().expect("dir");
    let (staging, target) = (root.path().join("server.staging"), root.path().join("server"));
    fs::create_dir_all(&staging).expect("staging");
    fs::write(staging.join(text::SERVER_EXE), b"exe").expect("exe");
    let cancelled = Ctl::detached();
    cancelled.cancel_for_test();
    assert_eq!(text::publish_server(&staging, &target, &cancelled), Err("Cancelled.".to_string()));
    assert!(!staging.exists() && !target.exists());

    fs::create_dir_all(&staging).expect("staging");
    fs::write(staging.join(text::SERVER_EXE), b"exe").expect("exe");
    let committed = Ctl::detached();
    assert_eq!(text::publish_server(&staging, &target, &committed), Ok(()));
    committed.cancel_for_test();
    assert!(!committed.is_cancelled(), "a committed download ignores a later cancel");
    assert!(target.join(text::SERVER_EXE).exists() && !staging.exists());
}

/// A staging folder without the server is removed and refused.
#[test]
fn text_publish_refuses_a_staging_folder_without_the_server() {
    let root = tempfile::tempdir().expect("dir");
    let (staging, target) = (root.path().join("server.staging"), root.path().join("server"));
    fs::create_dir_all(&staging).expect("staging");
    assert!(text::publish_server(&staging, &target, &Ctl::detached()).is_err());
    assert!(!staging.exists() && !target.exists());
}

/// A cancel that wins the commit point leaves the old pack in place and swaps nothing; a commit that wins swaps the new pack in.
#[test]
fn dictionary_publish_follows_the_commit_point() {
    let root = tempfile::tempdir().expect("dir");
    let (fresh, target) = (root.path().join("pack.tmp"), root.path().join("pack"));
    fs::create_dir_all(&fresh).expect("fresh");
    fs::create_dir_all(&target).expect("target");
    fs::write(target.join("old"), b"old").expect("old");
    fs::write(fresh.join(dictionary::DATABASE), b"new").expect("db");
    let cancelled = Ctl::detached();
    cancelled.cancel_for_test();
    assert_eq!(dictionary::publish_pack(&fresh, &target, &cancelled), Err("Cancelled.".to_string()));
    assert!(target.join("old").exists() && !target.join(dictionary::DATABASE).exists());

    assert_eq!(dictionary::publish_pack(&fresh, &target, &Ctl::detached()), Ok(()));
    assert!(target.join(dictionary::DATABASE).exists() && !target.join("old").exists());
}
