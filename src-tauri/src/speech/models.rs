//! Purpose: Tauri commands behind the Models page: list the catalog with install state, download, cancel, delete, choose.
//! Contents: list_models / download_model / cancel_download / delete_model — commands; ModelInfo — one row for the UI.

use serde::Serialize;
use tauri::AppHandle;

use super::catalog::{self, Model, MODELS};
use super::download;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    id: &'static str,
    name: &'static str,
    summary: &'static str,
    languages: &'static str,
    accuracy: u8,
    speed: u8,
    memory_mb: u32,
    recommended: bool,
    size: u64,
    installed: bool,
    downloading: bool,
}

fn info(app: &AppHandle, m: &'static Model) -> ModelInfo {
    ModelInfo {
        id: m.id,
        name: m.name,
        summary: m.summary,
        languages: m.languages,
        accuracy: m.accuracy,
        speed: m.speed,
        memory_mb: m.memory_mb,
        recommended: m.recommended,
        size: m.total_size(),
        installed: catalog::installed(app, m),
        downloading: download::is_active(m.id),
    }
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> Vec<ModelInfo> {
    MODELS.iter().map(|m| info(&app, m)).collect()
}

fn lookup(id: &str) -> Result<&'static Model, String> {
    catalog::find(id).ok_or_else(|| format!("Unknown model: {id}"))
}

#[tauri::command]
pub fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    download::start(&app, lookup(&id)?)
}

#[tauri::command]
pub fn cancel_download(id: String) {
    download::cancel(&id);
}

/// Stops the worker first: it may have the files open, and Windows will not delete an open file. Async so the wait for the worker
/// runs off the main thread (the speech thread may need the main thread to update the tray while it shuts down).
#[tauri::command]
pub async fn delete_model(app: AppHandle, id: String) -> Result<(), String> {
    let model = lookup(&id)?;
    if download::is_active(model.id) {
        return Err("Cancel the download first.".into());
    }
    super::unload();
    crate::platform::overlay::dismiss(); // unloading drops work in flight, which would leave the "processing" overlay up
    let dir = catalog::dir(&app, model).ok_or("Cue has no folder to store models in.")?;
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Could not delete the model: {e}")),
        _ => Ok(()),
    }
}
