//! File: model_download/tests/sizes.rs
//! Purpose: Protect the disk check made before a download starts: it counts only what is still to fetch and unpack, with headroom, and nothing for a file another model already installed.
//! Contents: tests of `sizes` over real files in a temp folder.

use std::fs;

use super::server::{data, file_of};
use crate::model_download::{sizes, Shared, Wanted};

/// A fresh file needs its whole size plus 5 % headroom; a partial file from an earlier run needs only the rest.
#[test]
fn a_partial_file_only_needs_room_for_what_is_missing() {
    let file = file_of("model.bin", &data(1000));
    let dir = tempfile::tempdir().expect("dir");
    let fresh = sizes(&[Wanted::own("https://example.invalid/m".into(), file, dir.path().to_path_buf())]);
    assert_eq!((fresh.total, fresh.need), (1000, 1050));
    fs::write(dir.path().join("model.bin.partial"), data(400)).expect("partial");
    let resumed = sizes(&[Wanted::own("https://example.invalid/m".into(), file, dir.path().to_path_buf())]);
    assert_eq!((resumed.total, resumed.need), (1000, 630));
}

/// A finished archive that was never unpacked still needs room to unpack, and nothing to fetch.
#[test]
fn a_complete_archive_needs_only_room_to_unpack() {
    let file = file_of("pack.zip", &data(1000));
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("pack.zip"), data(1000)).expect("archive");
    let mut wanted = Wanted::own("https://example.invalid/p".into(), file, dir.path().to_path_buf());
    wanted.unpacked = 2000;
    let room = sizes(&[wanted]);
    assert_eq!((room.total, room.need), (1000, 2100));
}

/// A shared file whose work another model already did costs nothing; the files of one model add up.
#[test]
fn a_shared_file_already_installed_costs_nothing_and_files_add_up() {
    let (vad, model) = (file_of("vad.onnx", &data(500)), file_of("model.bin", &data(1000)));
    let dir = tempfile::tempdir().expect("dir");
    let marker = dir.path().join("installed.marker");
    fs::write(&marker, b"done").expect("marker");
    let mut shared = Wanted::own("https://example.invalid/v".into(), vad, dir.path().to_path_buf());
    shared.shared = Some(Shared { skip_if: Some(marker), then: None });
    let own = Wanted::own("https://example.invalid/m".into(), model, dir.path().to_path_buf());
    let room = sizes(&[shared, own]);
    assert_eq!((room.total, room.need), (1500, 1050));
}
