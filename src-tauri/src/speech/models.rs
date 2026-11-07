//! Purpose: Tauri commands behind the Models page: list the catalog (speech models, the text model, then dictionary packs) with install state, download, cancel, delete.
//! Contents: list_models / download_model / cancel_download / delete_model — commands; Snapshot — the rows plus the revision of the download table they were read at; ModelInfo — one row for the UI (`kind` is "speech", "text" or "dictionary").

use serde::Serialize;
use tauri::AppHandle;

use super::catalog::{self, Model};
use super::catalog_stream;
use crate::dictionary::catalog as dictionary;
use crate::model_download::{self as download, View};
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
    /// The model needs more than a third of this PC's memory, so it would crowd other programs out.
    tight: bool,
    /// "long" or "short": whether the model is built for long sessions or for phrases (empty for text models).
    sessions: &'static str,
    size: u64,
    installed: bool,
    /// The running download of this model, if any.
    job: Option<Job>,
    /// Delete would clash with a running download (this model's own, or one that shares its files).
    delete_blocked: bool,
}

#[derive(Serialize)]
struct Job {
    generation: u64,
    done: u64,
    total: u64,
}

/// What the Models pages draw: every row, and the revision of the download table they were read at (newer events carry a higher one).
#[derive(Serialize)]
pub struct Snapshot {
    revision: u64,
    models: Vec<ModelInfo>,
}

/// The running download of `id` and whether deleting it is blocked, as the row shows them.
fn running(view: &View, id: &str, group: &str) -> (Option<Job>, bool) {
    let job = view.job(id).map(|j| Job { generation: j.generation, done: j.done, total: j.total });
    (job, view.delete_blocked(id, group))
}

fn tight(memory_mb: u32) -> bool {
    let ram = crate::platform::total_ram_mb();
    ram != 0 && memory_mb as u64 * 3 > ram
}

fn info(app: &AppHandle, view: &View, m: &'static Model) -> ModelInfo {
    let (job, delete_blocked) = running(view, m.id, "speech");
    ModelInfo {
        id: m.id,
        kind: "speech",
        name: m.name,
        summary: m.summary,
        languages: m.languages,
        accuracy: m.accuracy,
        speed: m.speed,
        memory_mb: m.memory_mb,
        recommended: m.id == super::default_model(),
        tight: tight(m.memory_mb),
        sessions: if m.long { "long" } else { "short" },
        size: m.total_size(),
        installed: catalog::installed(app, m),
        job,
        delete_blocked,
    }
}

fn text_info(app: &AppHandle, view: &View, m: &'static TextModel) -> ModelInfo {
    let (job, delete_blocked) = running(view, m.id, "text");
    ModelInfo {
        id: m.id,
        kind: "text",
        name: m.name,
        summary: m.summary,
        languages: m.languages,
        accuracy: m.accuracy,
        speed: m.speed,
        memory_mb: m.memory_mb,
        recommended: m.id == text::catalog::recommended(),
        tight: tight(m.memory_mb),
        sessions: "",
        size: text::catalog::total_size(app, m),
        installed: text::catalog::installed(app, m),
        job,
        delete_blocked,
    }
}

fn pack_info(app: &AppHandle, view: &View, p: &'static dictionary::Pack) -> ModelInfo {
    let (job, delete_blocked) = running(view, p.id, "dictionary");
    ModelInfo {
        id: p.id,
        kind: "dictionary",
        name: p.name,
        summary: p.summary,
        languages: p.name,
        accuracy: 0,
        speed: 0,
        memory_mb: 0,
        recommended: false,
        tight: false,
        sessions: "",
        size: p.file.size,
        installed: dictionary::installed(app, p),
        job,
        delete_blocked,
    }
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> Snapshot {
    let view = download::view();
    let models = catalog::all()
        .map(|m| info(&app, &view, m))
        .chain(text::catalog::MODELS.iter().map(|m| text_info(&app, &view, m)))
        .chain(dictionary::PACKS.iter().map(|p| pack_info(&app, &view, p)))
        .collect();
    Snapshot { revision: view.revision, models }
}

fn lookup(id: &str) -> Result<&'static Model, String> {
    catalog::find(id).ok_or_else(|| format!("Unknown model: {id}"))
}

#[tauri::command]
pub fn download_model(app: AppHandle, id: String) -> Result<download::Token, String> {
    if let Some(model) = text::catalog::find(&id) {
        let handle = app.clone();
        return download::start(&app, model.id, "text", Box::new(move || text::catalog::wanted(&handle, model)), Box::new(|_| Ok(())));
    }
    if let Some(pack) = dictionary::find(&id) {
        let handle = app.clone();
        let plan = Box::new(move || dictionary::wanted(&handle, pack));
        return download::start(&app, pack.id, "dictionary", plan, Box::new(move |app| dictionary::unpack(app, pack)));
    }
    let model = lookup(&id)?;
    let after: download::After = match model.archive {
        Some(_) => Box::new(move |app| catalog_stream::unpack(app, model)),
        None => Box::new(|_| Ok(())),
    };
    let handle = app.clone();
    download::start(&app, model.id, "speech", Box::new(move || catalog::wanted(&handle, model)), after)
}

#[tauri::command]
pub fn cancel_download(id: String, generation: u64) {
    download::cancel(&id, generation);
}

/// Stops the engine first: it may have the files open, and Windows will not delete an open file. Async so the wait for it
/// runs off the main thread (the speech thread may need the main thread to update the tray while it shuts down).
/// The model is reserved for the delete first, so a download cannot start into the files being removed; every window is told when it is over.
#[tauri::command]
pub async fn delete_model(app: AppHandle, id: String) -> Result<(), String> {
    let result = delete(&app, &id);
    download::announce(&app);
    result
}

/// The delete itself, under its reservation.
fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    if let Some(model) = text::catalog::find(id) {
        let _reserved = download::reserve_delete(model.id, "text")?;
        return text::stop_and_remove(app, model).map_err(|e| format!("Could not delete the model: {e}"));
    }
    if let Some(pack) = dictionary::find(id) {
        let _reserved = download::reserve_delete(pack.id, "dictionary")?;
        return dictionary::remove(app, pack).map_err(|e| format!("Could not delete the dictionary: {e}"));
    }
    let model = lookup(id)?;
    let _reserved = download::reserve_delete(model.id, "speech")?;
    super::unload();
    crate::platform::overlay::dismiss(); // unloading drops work in flight, which would leave the "processing" overlay up
    let dir = catalog::dir(app, model).ok_or("Cue has no folder to store models in.")?;
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Could not delete the model: {e}")),
        _ => Ok(()),
    }
}
