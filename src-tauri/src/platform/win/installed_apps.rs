//! Purpose: The programs installed on this PC, from the Start menu's "All apps" list (the shell folder `shell:AppsFolder`, the same source `Get-StartApps` reads): their display names for a picker, and a way to turn the path
//! of a running program back into that name, so a name picked from the list always matches the program it came from.
//! Contents: Catalog — the names and the lookups; catalog — the cached catalog; refresh — read the list again; entries — one pass over the shell folder; resolve — an AppID that points at a file, as a path.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use windows::core::GUID;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, SHGetKnownFolderItem, SHGetKnownFolderPath, KF_FLAG_DEFAULT, SIGDN,
    SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
};

use super::app_ids::{exe_stem, is_program, split_known_folder};
use super::com::Com;

/// What the list holds: every program's display name, and the display name by the file the program runs from (the whole path, and the file's name as a second chance when the path differs, as it does after an update).
#[derive(Default)]
pub struct Catalog {
    names: Vec<String>,
    by_path: HashMap<String, String>,
    by_exe: HashMap<String, String>,
}

impl Catalog {
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The display name of the program that runs from `path`, if the Start menu has it.
    pub fn name_for(&self, path: &str) -> Option<&str> {
        let by_path = self.by_path.get(&path.to_lowercase());
        by_path.or_else(|| exe_stem(path).and_then(|stem| self.by_exe.get(&stem))).map(String::as_str)
    }

    /// Whether some program of the list runs from an executable with this name.
    pub fn knows_exe(&self, stem: &str) -> bool {
        self.by_exe.contains_key(stem)
    }
}

static CATALOG: Mutex<Option<Arc<Catalog>>> = Mutex::new(None);

/// The catalog as last read; read on first use (a pass over the Start menu takes a fraction of a second).
pub fn catalog() -> Arc<Catalog> {
    let known = CATALOG.lock().unwrap_or_else(|e| e.into_inner()).clone();
    known.unwrap_or_else(refresh)
}

/// Reads the list again, so a program installed since shows up.
pub fn refresh() -> Arc<Catalog> {
    let mut built = Catalog::default();
    for (name, app_id) in entries().into_iter().filter(|(name, id)| is_program(name, id)) {
        if let Some(path) = resolve(&app_id) {
            built.by_path.insert(path.to_lowercase(), name.clone());
            if let Some(stem) = exe_stem(&path) {
                built.by_exe.entry(stem).or_insert_with(|| name.clone());
            }
        }
        built.names.push(name);
    }
    built.names.sort_by_key(|n| n.to_lowercase());
    built.names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    let built = Arc::new(built);
    *CATALOG.lock().unwrap_or_else(|e| e.into_inner()) = Some(built.clone());
    built
}

/// The text of one property of a shell item, handed back to Windows afterwards.
fn text(item: &IShellItem, kind: SIGDN) -> Option<String> {
    // SAFETY: the string is owned by COM until freed here, after it has been copied.
    unsafe {
        let raw = item.GetDisplayName(kind).ok()?;
        let copy = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0.cast()));
        copy
    }
}

/// Every entry of the "All apps" folder as its display name and its AppID.
fn entries() -> Vec<(String, String)> {
    let _com = Com::init();
    let mut found = Vec::new();
    // SAFETY: shell objects made and used on this thread while COM is initialised.
    unsafe {
        let Ok(folder) = SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) else { return found };
        let Ok(list) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else { return found };
        loop {
            let (mut batch, mut fetched) = ([None], 0u32);
            if list.Next(&mut batch, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = batch[0].take() else { break };
            let (Some(name), Some(parsing)) = (text(&item, SIGDN_NORMALDISPLAY), text(&item, SIGDN_DESKTOPABSOLUTEPARSING)) else {
                continue;
            };
            // The parsing name is `shell:AppsFolder\<AppID>`.
            let app_id = parsing.rsplit_once("AppsFolder\\").map_or(parsing.clone(), |(_, id)| id.to_string());
            found.push((name, app_id));
        }
    }
    found
}

/// An AppID that points at a file, as that file's path: an absolute path as it is, `{known folder}\rest` with the folder looked up. Store identities and plain identifiers have no path.
fn resolve(app_id: &str) -> Option<String> {
    if let Some((guid, rest)) = split_known_folder(app_id) {
        let folder = GUID::try_from(guid).ok()?;
        // SAFETY: the path string belongs to COM until freed here, after it has been copied.
        let base = unsafe {
            let raw = SHGetKnownFolderPath(&folder, KF_FLAG_DEFAULT, None).ok()?;
            let copy = raw.to_string().ok();
            CoTaskMemFree(Some(raw.0.cast()));
            copy
        }?;
        return Some(format!("{base}\\{rest}"));
    }
    app_id.get(1..3).filter(|drive| *drive == ":\\").map(|_| app_id.to_string())
}
