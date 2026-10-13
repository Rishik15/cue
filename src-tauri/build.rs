//! Purpose: Build script: embeds Tauri's resources and declares every command the app exposes to its windows, so each window can be granted only the commands it needs (see `capabilities/`).
//! Contents: COMMANDS — keep in step with `generate_handler!` in `src/lib.rs`; a command missing here is callable by every window.

const COMMANDS: &[&str] = &[
    "get_shortcut",
    "shortcut_record",
    "reset_shortcut",
    "get_settings",
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
    "preview_choose",
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

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("failed to run tauri-build");
}
