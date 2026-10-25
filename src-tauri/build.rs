//! Purpose: Build script: embeds Tauri's resources, embeds the common-controls manifest in every target, and declares every command the app exposes to its windows, so each window can be granted only the commands it needs (see `capabilities/`).
//! Contents: COMMANDS — keep in step with `generate_handler!` in `src/lib.rs`; a command missing here is callable by every window.

const COMMANDS: &[&str] = &[
    "get_shortcut",
    "shortcut_record",
    "shortcut_commit",
    "reset_shortcut",
    "get_settings",
    "get_theme",
    "set_setting",
    "get_autostart",
    "set_autostart",
    "remove_app_data",
    "open_link",
    "take_start_page",
    "check_update",
    "update_status",
    "install_update",
    "list_actions",
    "list_microphones",
    "preview_get",
    "preview_apply",
    "preview_close",
    "preview_run",
    "list_ocr_languages",
    "list_apps",
    "list_snippets",
    "save_snippet",
    "delete_snippet",
    "import_snippets",
    "export_snippets",
    "add_starter_snippets",
    "snippet_preview",
    "ask_get",
    "ask_submit",
    "note",
    "break_act",
    "save_folder",
    "delete_folder",
    "read_clipboard_text",
    "list_words",
    "delete_word",
    "get_rules",
    "save_rules",
    "test_rules",
    "rule_log",
    "get_stats",
    "list_models",
    "download_model",
    "cancel_download",
    "delete_model",
];

/// The dialog plugin imports comctl32 v6 entry points; the unit-test exe has no manifest and fails to load (0xc0000139). The linker embeds it for every target, so Tauri's own copy for the app binary is turned off.
fn link_common_controls() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
}

fn main() {
    link_common_controls();
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest).windows_attributes(windows))
        .expect("failed to run tauri-build");
}
