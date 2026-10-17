//! Purpose: Build script: embeds Tauri's resources, gives test exes the common-controls manifest, and declares every command the app exposes to its windows, so each window can be granted only the commands it needs (see `capabilities/`).
//! Contents: COMMANDS — keep in step with `generate_handler!` in `src/lib.rs`; a command missing here is callable by every window.

const COMMANDS: &[&str] = &[
    "get_shortcut",
    "shortcut_record",
    "reset_shortcut",
    "get_settings",
    "get_theme",
    "set_setting",
    "get_autostart",
    "set_autostart",
    "remove_app_data",
    "open_link",
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
    "save_folder",
    "delete_folder",
    "read_clipboard_text",
    "define_get",
    "define_fit",
    "define_save",
    "define_close",
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

/// The dialog plugin imports comctl32 v6 entry points; Tauri embeds that manifest only in the app binary, so the unit-test exe fails to load (0xc0000139) without it.
fn link_common_controls_for_tests() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
}

fn main() {
    link_common_controls_for_tests();
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("failed to run tauri-build");
}
