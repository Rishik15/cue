//! Purpose: The programs installed on this PC, from the Start menu's "All apps" list (the shell folder `shell:AppsFolder`, the same source `Get-StartApps` reads): their display names for a picker, each with the executable it runs,
//! so a pick is saved with the executable that the focus check later compares.
//! Contents: Program / programs — the names with the executable each runs, read on demand; entries — one pass over the shell folder; resolve — an AppID that points at a file, as a path.

use windows::core::GUID;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, SHGetKnownFolderItem, SHGetKnownFolderPath, KF_FLAG_DEFAULT, SIGDN,
    SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
};

use super::app_ids::{exe_stem, is_program, split_known_folder};
use super::com::Com;

/// A program of the list: the name people know it by and the executable it runs (its file name without `.exe`, lower case), when the entry points at a file.
pub struct Program {
    pub name: String,
    pub exe: Option<String>,
}

/// The Start menu's programs, read fresh on every call and not kept: only the picker needs them, and the focus check compares executables.
pub fn programs() -> Vec<Program> {
    let mut found: Vec<Program> = Vec::new();
    for (name, app_id) in entries().into_iter().filter(|(name, id)| is_program(name, id)) {
        let exe = resolve(&app_id).and_then(|path| exe_stem(&path));
        match found.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&name)) {
            Some(same) => same.exe = same.exe.take().or(exe),
            None => found.push(Program { name, exe }),
        }
    }
    found
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
