//! Purpose: The dictionary packs Cue offers (one per language, glosses in English): where each is downloaded from, its pinned size and SHA-256, and where it lives on disk.
//! Contents: Pack / PACKS — the list; find / installed / installed_packs — lookups; path — the unpacked database; wanted / unpack / remove — download, unpack and delete one pack.
//! A pack is a zip of one `dictionary.db`, published as a GitHub release asset. Nothing ships inside the app.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::speech::catalog::ModelFile;
use crate::speech::download::Wanted;

pub struct Pack {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub url: &'static str,
    pub file: ModelFile,
}

/// English from Open English WordNet 2025, built with `scripts/build-dictionary.mjs` and published as a release asset of this repository (tag `dictionary-1`).
pub const PACKS: &[Pack] = &[Pack {
    id: "dictionary-en",
    name: "English",
    summary: "Meanings, examples and synonyms for 127,000 words",
    url: "https://github.com/Rishik15/cue/releases/download/dictionary-1/dictionary-en.zip",
    file: ModelFile {
        name: "dictionary-en.zip",
        size: 8_657_994,
        sha256: "b969a464c4a30c3fe334be2e8c4319bbbb53ed8a639cafe8747e484e91f13599",
    },
}];

const DATABASE: &str = "dictionary.db";

pub fn find(id: &str) -> Option<&'static Pack> {
    PACKS.iter().find(|p| p.id == id)
}

/// Under the models folder, so "Remove All Cue Data" deletes it too.
fn root(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_local_data_dir().ok()?.join("models").join("dictionary"))
}

fn dir(app: &AppHandle, pack: &Pack) -> Option<PathBuf> {
    Some(root(app)?.join(pack.id))
}

pub fn path(app: &AppHandle, pack: &Pack) -> Option<PathBuf> {
    Some(dir(app, pack)?.join(DATABASE))
}

pub fn installed(app: &AppHandle, pack: &Pack) -> bool {
    path(app, pack).is_some_and(|p| p.is_file())
}

pub fn installed_packs(app: &AppHandle) -> Vec<&'static Pack> {
    PACKS.iter().filter(|p| installed(app, p)).collect()
}

pub fn wanted(app: &AppHandle, pack: &'static Pack) -> Result<Vec<Wanted>, String> {
    let root = root(app).ok_or("Cue has no folder to store dictionaries in.")?;
    Ok(vec![Wanted { url: pack.url.into(), file: &pack.file, dir: root.join("archive") }])
}

/// Unpacks the downloaded zip into the pack's folder, then deletes the archive.
pub fn unpack(app: &AppHandle, pack: &Pack) -> Result<(), String> {
    let root = root(app).ok_or("Cue has no folder to store dictionaries in.")?;
    let (archive, target) = (root.join("archive").join(pack.file.name), root.join(pack.id));
    crate::platform::extract_zip(&archive, &target)?;
    if !target.join(DATABASE).is_file() {
        return Err("The dictionary file was not in the download. Try again.".into());
    }
    let _ = std::fs::remove_dir_all(root.join("archive"));
    Ok(())
}

pub fn remove(app: &AppHandle, pack: &Pack) -> std::io::Result<()> {
    match dir(app, pack).map(std::fs::remove_dir_all) {
        Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
