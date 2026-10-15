//! Purpose: Reading the identifiers Windows gives the entries of the Start menu's "All apps" list (its AppIDs), which come in several shapes. Pure text work, so it is tested without Windows.
//! Contents: is_program — whether an entry is a program and not a web link, document or uninstaller; split_known_folder — `{GUID}\rest` into the folder's GUID and the rest; exe_stem — the executable's name from a path.

/// Entries of the list that are not programs: web links, documents, management snap-ins and uninstallers.
pub fn is_program(name: &str, app_id: &str) -> bool {
    let id = app_id.to_lowercase();
    let links = ["http://", "https://"].iter().any(|p| id.starts_with(p));
    let documents = [".url", ".html", ".htm", ".chm", ".pdf", ".txt", ".rtf", ".md", ".msc", ".lnk"].iter().any(|e| id.ends_with(e));
    !name.trim().is_empty() && !links && !documents && !name.to_lowercase().starts_with("uninstall")
}

/// `{6D809377-6AF0-444B-8957-A3773F02200E}\Git\cmd\git-gui.exe` to the folder's GUID and `Git\cmd\git-gui.exe`; None for any other shape.
pub fn split_known_folder(app_id: &str) -> Option<(&str, &str)> {
    let rest = app_id.strip_prefix('{')?;
    let (guid, tail) = rest.split_once("}\\")?;
    (guid.len() == 36).then_some((&app_id[..guid.len() + 2], tail))
}

/// The executable's name without `.exe`, lower case, from a path; None when the path is not an executable.
pub fn exe_stem(path: &str) -> Option<String> {
    let file = path.rsplit(['\\', '/']).next()?.to_lowercase();
    file.strip_suffix(".exe").filter(|stem| !stem.is_empty()).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_are_told_from_links_documents_and_uninstallers() {
        assert!(
            is_program("DBeaver", r"C:\Users\a\dbeaver.exe")
                && is_program("Discord", "com.rishik.pake.discord")
                && is_program("Calculator", "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App")
        );
        assert!(
            !is_program("Node.js website", "https://nodejs.org/")
                && !is_program("Git Release Notes", r"{6D809377-6AF0-444B-8957-A3773F02200E}\Git\ReleaseNotes.html")
        );
        assert!(
            !is_program("Component Services", r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\comexp.msc")
                && !is_program("Uninstall", r"C:\x\Uninstall.exe")
                && !is_program(" ", "x")
        );
    }

    #[test]
    fn a_known_folder_path_splits_into_its_guid_and_the_rest() {
        assert_eq!(
            split_known_folder(r"{6D809377-6AF0-444B-8957-A3773F02200E}\glzr.io\GlazeWM\glazewm.exe"),
            Some(("{6D809377-6AF0-444B-8957-A3773F02200E}", r"glzr.io\GlazeWM\glazewm.exe"))
        );
        assert_eq!(
            (split_known_folder("com.pais.handy"), split_known_folder(r"C:\a\b.exe"), split_known_folder(r"{short}\x")),
            (None, None, None)
        );
    }

    #[test]
    fn the_executable_name_comes_from_a_path() {
        assert_eq!(exe_stem(r"C:\Program Files\Zed\Zed.EXE").as_deref(), Some("zed"));
        assert_eq!((exe_stem(r"C:\a\readme.html"), exe_stem(".exe")), (None, None));
    }
}
