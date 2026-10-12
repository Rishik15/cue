//! Purpose: Open a web address in the user's default browser.
//! Contents: open_url — hands the address to the URL protocol handler, which uses the default browser. Callers pass only addresses of their own, never user input.
//! `explorer.exe <url>` opens a folder window for any address with a `?` query, so the handler is called directly.
use std::process::Command;

pub fn open_url(url: &str) -> Result<(), String> {
    Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open the browser: {e}"))
}
