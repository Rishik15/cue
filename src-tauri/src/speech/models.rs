//! Purpose: Tauri commands behind the Models page: list the catalog (speech models, then the text model) with install state, download, cancel, delete.
//! Contents: list_models / download_model / cancel_download / delete_model — commands; ModelInfo — one row for the UI (`kind` is "speech" or "text").

use serde::Serialize;
use tauri::AppHandle;

use super::catalog::{self, Model};
use super::catalog_stream;
use super::download;
use crate::text::{self, catalog::TextModel};

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
        kind: if m.is_streaming() { "stream" } else { "speech" },
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

fn text_info(app: &AppHandle, m: &'static TextModel) -> ModelInfo {
    ModelInfo {
        id: m.id,
        kind: "text",
        name: m.name,
        summary: m.summary,
        languages: m.languages,
        accuracy: m.accuracy,
        speed: m.speed,
        memory_mb: m.memory_mb,
        recommended: m.recommended,
        size: text::catalog::total_size(app, m),
        installed: text::catalog::installed(app, m),
        downloading: download::is_active(m.id),
    }
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> Vec<ModelInfo> {
    catalog::all().map(|m| info(&app, m)).chain(text::catalog::MODELS.iter().map(|m| text_info(&app, m))).collect()
}

fn lookup(id: &str) -> Result<&'static Model, String> {
    catalog::find(id).ok_or_else(|| format!("Unknown model: {id}"))
}

#[tauri::command]
pub fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(model) = text::catalog::find(&id) {
        return download::start(&app, model.id, text::catalog::wanted(&app, model)?, Box::new(text::catalog::unpack));
    }
    let model = lookup(&id)?;
    let after: download::After = match model.archive {
        Some(_) => Box::new(move |app| catalog_stream::unpack(app, model)),
        None => Box::new(|_| Ok(())),
    };
    download::start(&app, model.id, catalog::wanted(&app, model)?, after)
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
    if let Some(model) = text::catalog::find(&id) {
        text::stop();
        return text::catalog::remove(&app, model).map_err(|e| format!("Could not delete the model: {e}"));
    }
    super::unload();
    crate::platform::overlay::dismiss(); // unloading drops work in flight, which would leave the "processing" overlay up
    let dir = catalog::dir(&app, lookup(&id)?).ok_or("Cue has no folder to store models in.")?;
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Could not delete the model: {e}")),
        _ => Ok(()),
    }
}
