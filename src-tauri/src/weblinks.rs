//! Purpose: The few web pages the About page links to. The page names a link, never an address, so the webview cannot make Cue open arbitrary URLs.
//! Contents: open_link — command: open the repository, the feedback form or the issue form in the default browser.
use crate::platform;

const REPO: &str = "https://github.com/Rishik15/cue";

#[tauri::command]
pub fn open_link(which: String) -> Result<(), String> {
    let url = match which.as_str() {
        "repo" => REPO.to_string(),
        "feedback" => format!("{REPO}/issues/new?labels=feedback&title=Feedback%3A%20"),
        "issue" => format!("{REPO}/issues/new?labels=bug"),
        _ => return Err(format!("Unknown link: {which}")),
    };
    platform::open_url(&url)
}
