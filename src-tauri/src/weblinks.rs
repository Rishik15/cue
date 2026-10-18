//! Purpose: The few web pages the About page links to. The page names a link, never an address, so the webview cannot make Cue open arbitrary URLs.
//! Contents: open_link — command: open the repository, the feedback form or the issue form in the default browser, the crash log in the default text viewer, or Windows' language settings.
use crate::platform;

const REPO: &str = "https://github.com/Rishik15/cue";

#[tauri::command]
pub fn open_link(app: tauri::AppHandle, which: String) -> Result<(), String> {
    let url = match which.as_str() {
        "crash" => return open_crash_log(&app),
        "language" => "ms-settings:regionlanguage".to_string(),
        "repo" => REPO.to_string(),
        "wiktionary" => "https://en.wiktionary.org".to_string(),
        "feedback" => format!("{REPO}/issues/new?labels=feedback&title=Feedback%3A%20"),
        "issue" => format!("{REPO}/issues/new?labels=bug"),
        _ => return Err(format!("Unknown link: {which}")),
    };
    platform::open_url(&url)
}

/// The file Cue writes when it recovers from a bug (`guard`); opened in whatever the user has for text files.
fn open_crash_log(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let log = app.path().app_data_dir().map_err(|e| e.to_string())?.join("crash.log");
    if !log.exists() {
        return Err("Cue has not recorded any problem.".into());
    }
    platform::open_url(&log.to_string_lossy())
}
