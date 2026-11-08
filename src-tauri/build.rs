//! File: build.rs
//! Purpose: Build script: embeds Tauri's resources, embeds the common-controls manifest in every target, and holds the one declaration of every command the app exposes and the windows allowed to call it. The build fails when `capabilities/` grants or the registered handlers differ from that declaration.
//! Contents: COMMANDS — command name and the windows granted it; a command missing here is callable by every window;
//! handler_names / check_handler — the build fails unless `generate_handler!` in `src/lib.rs` registers exactly these; grants_of / check_grants — one capability file's windows and commands (parsed as JSON) and exact per-window grant validation; link_common_controls; main.

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

/// The commands `generate_handler!` registers, read from `src/lib.rs`: the last path segment of each entry. Input: the source text. Returns none when the macro is missing, which the caller reports.
fn handler_names(source: &str) -> BTreeSet<String> {
    let Some(start) = source.find("generate_handler![") else { return BTreeSet::new() };
    let body = &source[start + "generate_handler![".len()..];
    let body = &body[..body.find(']').unwrap_or(body.len())];
    body.split(',').filter_map(|entry| entry.trim().rsplit("::").next()).filter(|name| !name.is_empty()).map(String::from).collect()
}

/// The windows of one capability file and the commands it grants (`allow-some-command` as `some_command`). Input: the parsed file. A permission is a string or an object with an `identifier`; scoped objects count by their identifier.
fn grants_of(file: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let strings = |key: &str| {
        file[key].as_array().into_iter().flatten().filter_map(|v| v.as_str().or_else(|| v["identifier"].as_str())).map(String::from)
    };
    let commands = strings("permissions").filter_map(|p| p.strip_prefix("allow-").map(|name| name.replace('-', "_"))).collect();
    (strings("windows").collect(), commands)
}

/// Fails the build when `generate_handler!` registers a command `COMMANDS` does not name, or the reverse, so a command can never be callable by every window because it was left out of the manifest.
fn check_handler(root: &Path) {
    let lib = root.join("src").join("lib.rs");
    println!("cargo:rerun-if-changed={}", lib.display());
    let registered = handler_names(&std::fs::read_to_string(&lib).expect("src/lib.rs"));
    let declared: BTreeSet<String> = COMMANDS.iter().map(|(name, _)| (*name).to_string()).collect();
    assert!(
        registered == declared,
        "generate_handler! and COMMANDS differ: only registered {:?}, only declared {:?}",
        registered.difference(&declared).collect::<Vec<_>>(),
        declared.difference(&registered).collect::<Vec<_>>()
    );
}

/// Fails the build when the commands granted to any window differ from `COMMANDS`, so a command can neither be missing from the manifest nor reach a window it was not declared for.
fn check_grants(root: &Path) {
    let dir = root.join("capabilities");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut granted: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).expect("capabilities folder") {
        let text = std::fs::read_to_string(entry.expect("capability entry").path()).expect("capability file");
        let file: serde_json::Value = serde_json::from_str(&text).expect("capability file is JSON");
        let (windows, commands) = grants_of(&file);
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
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    check_handler(&root);
    check_grants(&root);
    let names: &'static [&'static str] = Box::leak(COMMANDS.iter().map(|(name, _)| *name).collect::<Vec<_>>().into_boxed_slice());
    let manifest = tauri_build::AppManifest::new().commands(names);
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest).windows_attributes(windows))
        .expect("failed to run tauri-build");
}
