//! Purpose: Tauri commands behind the Models page: list the catalog (speech models, then the text model) with install state, download, cancel, delete.
//! Contents: list_models / download_model / cancel_download / delete_model — commands; ModelInfo — one row for the UI (`kind` is "speech" or "text").

use serde::Serialize;
use tauri::AppHandle;

use super::catalog::{self, Model, MODELS};
use super::download;
use crate::text;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    id: &'static str,
    kind: &'static str,
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
        kind: "speech",
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

/// The text model has no accuracy or speed rating (0 hides the meters): it has not been measured the way the speech models were.
fn text_info(app: &AppHandle) -> ModelInfo {
    ModelInfo {
        id: text::catalog::ID,
        kind: "text",
        name: text::catalog::NAME,
        summary: text::catalog::SUMMARY,
        languages: "English and more",
        accuracy: 0,
        speed: 0,
        memory_mb: text::catalog::MEMORY_MB,
        recommended: false,
        size: text::catalog::total_size(app),
        installed: text::catalog::installed(app),
        downloading: download::is_active(text::catalog::ID),
    }
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> Vec<ModelInfo> {
    MODELS.iter().map(|m| info(&app, m)).chain(std::iter::once(text_info(&app))).collect()
}

fn lookup(id: &str) -> Result<&'static Model, String> {
    catalog::find(id).ok_or_else(|| format!("Unknown model: {id}"))
}

#[tauri::command]
pub fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    if id == text::catalog::ID {
        return download::start(&app, text::catalog::ID, text::catalog::wanted(&app)?, text::catalog::unpack);
    }
    let model = lookup(&id)?;
    download::start(&app, model.id, catalog::wanted(&app, model)?, |_| Ok(()))
}

#[tauri::command]
pub fn cancel_download(id: String) {
    download::cancel(&id);
}

/// Stops the engine first: it may have the files open, and Windows will not delete an open file. Async so the wait for it
/// runs off the main thread (the speech thread may need the main thread to update the tray while it shuts down).
#[tauri::command]
pub async fn delete_model(app: AppHandle, id: String) -> Result<(), String> {
    if download::is_active(&id) {
        return Err("Cancel the download first.".into());
    }
    let dir = if id == text::catalog::ID {
        text::stop();
        text::catalog::dir(&app)
    } else {
        super::unload();
        crate::platform::overlay::dismiss(); // unloading drops work in flight, which would leave the "processing" overlay up
        catalog::dir(&app, lookup(&id)?)
    };
    match std::fs::remove_dir_all(dir.ok_or("Cue has no folder to store models in.")?) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Could not delete the model: {e}")),
        _ => Ok(()),
    }
}
