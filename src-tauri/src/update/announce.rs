//! Purpose: Say "Cue updated to x.y.z" once after an update, by remembering the last version that ran.
//! Contents: should_announce — pure rule (a different, known previous version); announce_if_updated — read the marker file, show the pill, write the current version.

use tauri::{AppHandle, Manager};

use crate::platform::overlay;
use crate::prefs::Prefs;

/// A first install (no marker) or the same version says nothing.
fn should_announce(previous: Option<&str>, current: &str) -> bool {
    previous.map(str::trim).is_some_and(|p| !p.is_empty() && p != current)
}

/// A plain file, not the settings store: saving a setting would start the store plugin's async runtime and add idle threads.
pub fn announce_if_updated(app: &AppHandle) {
    let Ok(dir) = app.path().app_data_dir() else { return };
    let file = dir.join("last-version");
    let current = app.package_info().version.to_string();
    let previous = std::fs::read_to_string(&file).ok();
    // The marker goes first: if it cannot be written, saying nothing beats repeating the notice at every launch.
    if previous.as_deref().map(str::trim) != Some(current.as_str())
        && std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, &current)).is_err()
    {
        return;
    }
    if should_announce(previous.as_deref(), &current) {
        let prefs = Prefs::load(app);
        overlay::notice(&format!("Cue updated to {current}"), prefs.overlay_top, prefs.theme());
    }
}

#[cfg(test)]
mod tests {
    use super::should_announce;

    #[test]
    fn only_a_changed_known_version_is_announced() {
        assert!(should_announce(Some("0.1.1\n"), "0.2.0"));
        assert!(!should_announce(Some("0.2.0"), "0.2.0"));
        assert!(!should_announce(Some(""), "0.2.0"));
        assert!(!should_announce(None, "0.2.0"));
    }
}
