//! File: build.rs
//! Purpose: Build script: embeds Tauri's resources, embeds the common-controls manifest in every target, and holds the one declaration of every command the app exposes and the windows allowed to call it. The build fails when `capabilities/` grants differ from that declaration.
//! Contents: COMMANDS — command name and the windows granted it, keep the names in step with `generate_handler!` in `src/lib.rs`; a command missing here is callable by every window;
//! quoted — the quoted strings of a JSON list; grants_of — one capability file's windows and commands; check_grants — exact per-window grant validation; link_common_controls; main.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const COMMANDS: &[(&str, &[&str])] = &[
    ("get_shortcut", &["settings"]),
    ("shortcut_record", &["settings"]),
    ("shortcut_record_cancel", &["settings"]),
    ("reset_shortcut", &["settings"]),
    ("get_settings", &["settings"]),
    ("get_theme", &["ask", "preview"]),
    ("set_setting", &["settings"]),
    ("get_autostart", &["settings"]),
    ("set_autostart", &["settings"]),
    ("remove_app_data", &["settings"]),
    ("open_link", &["settings"]),
    ("take_start_page", &["settings"]),
    ("check_update", &["settings"]),
    ("update_status", &["settings"]),
    ("install_update", &["settings"]),
    ("list_actions", &["settings"]),
    ("list_microphones", &["settings"]),
    ("preview_get", &["preview"]),
    ("preview_apply", &["preview"]),
    ("preview_close", &["preview"]),
    ("preview_run", &["preview"]),
    ("list_ocr_languages", &["settings"]),
    ("list_apps", &["settings"]),
    ("list_snippets", &["settings"]),
    ("save_snippet", &["settings"]),
    ("delete_snippet", &["settings"]),
    ("import_snippets", &["settings"]),
    ("export_snippets", &["settings"]),
    ("add_starter_snippets", &["settings"]),
    ("snippet_preview", &["settings"]),
    ("ask_get", &["ask"]),
    ("ask_submit", &["ask"]),
    ("note", &["notes"]),
    ("break_act", &["break"]),
    ("save_folder", &["settings"]),
    ("delete_folder", &["settings"]),
    ("read_clipboard_text", &["settings"]),
    ("list_words", &["settings"]),
    ("delete_word", &["settings"]),
    ("get_rules", &["settings"]),
    ("save_rules", &["settings"]),
    ("test_rules", &["settings"]),
    ("rule_log", &["settings"]),
    ("get_stats", &["settings"]),
    ("list_models", &["settings"]),
    ("download_model", &["settings"]),
    ("cancel_download", &["settings"]),
    ("delete_model", &["settings"]),
];

/// The quoted strings of the first JSON list after `key` in `text`, or none when the key or list is missing.
fn quoted(text: &str, key: &str) -> Vec<String> {
    let Some(at) = text.find(key) else { return Vec::new() };
    let rest = &text[at + key.len()..];
    let (Some(open), Some(close)) = (rest.find('['), rest.find(']')) else { return Vec::new() };
    rest[open + 1..close].split('"').skip(1).step_by(2).map(String::from).collect()
}

/// The windows of one capability file and the commands it grants (`allow-some-command` as `some_command`).
fn grants_of(text: &str) -> (Vec<String>, Vec<String>) {
    let commands =
        quoted(text, "\"permissions\"").into_iter().filter_map(|p| p.strip_prefix("allow-").map(|name| name.replace('-', "_"))).collect();
    (quoted(text, "\"windows\""), commands)
}

/// Fails the build when the commands granted to any window differ from `COMMANDS`, so a command can neither be missing from the manifest nor reach a window it was not declared for.
fn check_grants() {
    let dir = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("capabilities");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut granted: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).expect("capabilities folder") {
        let text = std::fs::read_to_string(entry.expect("capability entry").path()).expect("capability file");
        let (windows, commands) = grants_of(&text);
        for window in windows {
            granted.entry(window).or_default().extend(commands.iter().cloned());
        }
    }
    let mut declared: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (command, windows) in COMMANDS {
        for window in *windows {
            declared.entry((*window).to_string()).or_default().insert((*command).to_string());
        }
    }
    for (window, commands) in &declared {
        let have = granted.get(window).cloned().unwrap_or_default();
        assert!(*commands == have, "window {window}: capabilities grant {have:?} but COMMANDS declares {commands:?}");
    }
    let unknown: Vec<_> = granted.keys().filter(|w| !declared.contains_key(*w)).collect();
    assert!(unknown.is_empty(), "capabilities grant commands to windows COMMANDS does not name: {unknown:?}");
}

/// The dialog plugin imports comctl32 v6 entry points; the unit-test exe has no manifest and fails to load (0xc0000139). The linker embeds it for every target, so Tauri's own copy for the app binary is turned off.
fn link_common_controls() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
}

/// Checks the grants, then runs Tauri's build step with the declared commands as the app manifest.
fn main() {
    link_common_controls();
    check_grants();
    let names: &'static [&'static str] = Box::leak(COMMANDS.iter().map(|(name, _)| *name).collect::<Vec<_>>().into_boxed_slice());
    let manifest = tauri_build::AppManifest::new().commands(names);
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest).windows_attributes(windows))
        .expect("failed to run tauri-build");
}
