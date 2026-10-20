//! Purpose: What a new install and an update show in the note window: a real first note that stays the user's, and the what's-new page after an update.
//! Contents: seed — put the intro in the first note; pending — the marker file `notes-seen` against the running version; should_show — the pure rule; INTRO — the text of the first note (it holds no `timer` or `autopaste` line, which would start on their own).

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use super::{store, Note};

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

/// Writes the intro into the empty first note of a new install (a note that already has text is left alone).
pub fn seed(c: &Connection, note: &mut Note) -> Result<(), String> {
    if note.body.is_empty() {
        store::save(c, note.id, INTRO)?;
        note.body = INTRO.into();
    }
    Ok(())
}

/// The first note of a new install. Ordinary text from then on: the user may edit, keep or delete it.
pub const INTRO: &str = "Welcome to quick notes
Plain text. Esc closes and it is saved. Ctrl+P lists your notes, Ctrl+/ shows the keys.

Math: end a line with =
12 x 8 =
rent: 900
rent / 4 =
10 ft to m

Checklist: type [ ] and press Ctrl+Enter to tick
[ ] try a checklist

Timer: type timer 5m on a line, then move off it
Pomodoro: type timer pomodoro (25 min focus, 5 rest, no end) or timer pomodoro x4 (four rounds)
or timer 50 10 for 50 minutes of focus and 10 of rest
Click the strip on the left to pause, double-click to stop.
Settings, Notes sets the alert: notification, sound or full screen.
";

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
