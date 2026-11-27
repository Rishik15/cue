//! File: settings_door/mod.rs
//! Purpose: The settings window's write door: checks a write against the allow-list, saves it through `settings::update` and carries out the effects the key has, so `settings` itself stays a store that knows no feature.
//! Contents: set_setting — the window's write command; validate — the allow-list; effects — what each saved key changes.

mod effects;
mod validate;

use serde_json::Value;
use tauri::AppHandle;

pub use validate::validate;

/// Writes one validated setting from the UI and applies its effects. Inputs: key and value. Returns Err with a message when validation fails, the settings are read-only or the save fails; the UI restores its previous value then.
#[tauri::command]
pub async fn set_setting(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    validate(&key, &value)?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::snippets::check_setting_change(&app, &key, &value)?;
        crate::settings::update(&[(key.as_str(), Some(value))])?;
        for effect in effects::effects(&key) {
            effects::apply(&app, *effect);
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests;
