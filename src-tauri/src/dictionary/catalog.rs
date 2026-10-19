//! Purpose: The dictionary packs Cue offers (one per language, glosses in English): where each is downloaded from, its pinned size and SHA-256, and where it lives on disk.
//! Contents: Pack / PACKS — the list; find / installed / installed_packs — lookups; path — the unpacked database; wanted / unpack / swap_in / remove — download, unpack (into a temp folder, then swapped in) and delete one pack.
//! A pack is a zip of one `dictionary.db`, kept in this repository (`packs/`) and downloaded from its raw address. Nothing ships inside the app or the installer.

use std::path::{Path, PathBuf};

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

/// English from Wiktionary (CC BY-SA 4.0, extracted by kaikki.org), built with `scripts/build-dictionary.mjs` and committed to this repository as `packs/dictionary-en.zip` (not a release). A new build changes its hash, so update the size and SHA-256 here when `packs/dictionary-en.zip` is replaced.
pub const PACKS: &[Pack] = &[Pack {
    id: "dictionary-en",
    name: "English",
    summary: "Meanings, examples and synonyms for 1.3 million words",
    url: "https://raw.githubusercontent.com/Rishik15/cue/main/packs/dictionary-en.zip",
    file: ModelFile {
        name: "dictionary-en.zip",
        size: 52_299_636,
        sha256: "9de2b5d62116646273ae55c765898a7af32c3ecc4b89512dd7f8604d1911d067",
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

/// Unpacks the downloaded zip next to the pack's folder, checks it, then swaps it in, so a failed or partial unpack never looks installed. The archive goes only after success.
pub fn unpack(app: &AppHandle, pack: &Pack) -> Result<(), String> {
    let root = root(app).ok_or("Cue has no folder to store dictionaries in.")?;
    let archive = root.join("archive");
    let fresh = root.join(format!("{}.tmp", pack.id));
    let _ = std::fs::remove_dir_all(&fresh);
    let result = crate::platform::extract_zip(&archive.join(pack.file.name), &fresh).and_then(|()| swap_in(&fresh, &root.join(pack.id)));
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&fresh);
    } else {
        let _ = std::fs::remove_dir_all(&archive); // kept on failure so a retry does not download it again
    }
    result
}

/// Replaces `target` with `fresh` when `fresh` holds the database.
fn swap_in(fresh: &Path, target: &Path) -> Result<(), String> {
    if !fresh.join(DATABASE).is_file() {
        return Err("The dictionary file was not in the download. Try again.".into());
    }
    // The old pack is moved aside, not deleted, so a failed rename (antivirus, indexer) can put it back.
    let aside = target.with_extension("old");
    let _ = std::fs::remove_dir_all(&aside);
    let had_old = match std::fs::rename(target, &aside) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("Could not replace the old dictionary: {e}")),
        result => result.is_ok(),
    };
    if let Err(e) = std::fs::rename(fresh, target) {
        if had_old {
            let _ = std::fs::rename(&aside, target);
        }
        return Err(format!("Could not install the dictionary: {e}"));
    }
    let _ = std::fs::remove_dir_all(&aside);
    Ok(())
}

/// Deletes the pack, any half-finished download, and the dictionary folder once nothing is left in it.
pub fn remove(app: &AppHandle, pack: &Pack) -> std::io::Result<()> {
    let (Some(root), Some(dir)) = (root(app), dir(app, pack)) else { return Ok(()) };
    let _ = std::fs::remove_dir_all(root.join("archive"));
    let result = match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    };
    let _ = std::fs::remove_dir(root); // only succeeds when empty
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str, file: Option<&str>) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cue-catalog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(file) = file {
            std::fs::write(dir.join(file), file).unwrap();
        }
        dir
    }

    #[test]
    fn a_complete_unpack_replaces_the_old_pack() {
        let (fresh, old) = (folder("fresh", Some(DATABASE)), folder("old", Some("stale")));
        swap_in(&fresh, &old).unwrap();
        assert!(old.join(DATABASE).is_file() && !old.join("stale").exists() && !fresh.exists());
        let _ = std::fs::remove_dir_all(old);
    }

    #[test]
    fn an_unpack_without_the_database_leaves_the_old_pack_alone() {
        let (fresh, old) = (folder("empty", Some("other")), folder("keep", Some(DATABASE)));
        assert!(swap_in(&fresh, &old).is_err());
        assert!(old.join(DATABASE).is_file());
        let _ = (std::fs::remove_dir_all(fresh), std::fs::remove_dir_all(old));
    }
}
