//! Purpose: Decide whether the note window opens with the guide page: on the first open and on the first open after an update.
//! Contents: pending — the marker file `notes-seen` against the running version; should_show — the pure rule.

use tauri::{AppHandle, Manager};

/// What the page shows: "first" for a new install, else the version it was updated to; None when this version was seen.
fn should_show(seen: Option<&str>, current: &str) -> Option<String> {
    match seen.map(str::trim) {
        Some(v) if v == current => None,
        Some(v) if !v.is_empty() => Some(current.to_string()),
        _ => Some("first".into()),
    }
}

/// A plain file beside `last-version` (that one is rewritten at launch, before any note opens). The marker is written first: a failure to write it must not repeat the page at every open.
pub fn pending(app: &AppHandle) -> Option<String> {
    let dir = app.path().app_data_dir().ok()?;
    let file = dir.join("notes-seen");
    let current = app.package_info().version.to_string();
    let show = should_show(std::fs::read_to_string(&file).ok().as_deref(), &current)?;
    std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, &current)).ok()?;
    Some(show)
}

#[cfg(test)]
mod tests {
    use super::should_show;

    #[test]
    fn shows_on_first_open_and_after_an_update_only() {
        assert_eq!(should_show(None, "0.2.0").as_deref(), Some("first"));
        assert_eq!(should_show(Some(""), "0.2.0").as_deref(), Some("first"));
        assert_eq!(should_show(Some("0.1.1\n"), "0.2.0").as_deref(), Some("0.2.0"));
        assert_eq!(should_show(Some("0.2.0"), "0.2.0"), None);
    }
}
