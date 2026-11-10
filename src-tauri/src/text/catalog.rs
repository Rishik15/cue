//! Purpose: The text models Cue offers for Correct and Rewrite, and the llama.cpp server that runs them: pinned URLs, sizes and SHA-256, where they live, how the server archive is unpacked.
//! Contents: TextModel / MODELS — the curated list with the facts the Models page shows; find / current — lookups (the chosen model if installed, else any installed one); wanted — the files a download fetches;
//! installed — model and server are on disk; model_path / server_path / model_dir — locations; unpack — extract the server archive (staged, then renamed); recover — drop a half-unpacked server; remove — delete one model (and the server with the last one).
//! The server is shared by all models: it is downloaded once with the first model.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::model_download::{run_child, Ctl, Shared, Wanted};
use crate::speech::catalog::ModelFile;

pub struct TextModel {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub languages: &'static str,
    /// 1 to 5 from `scripts/cue.ps1 lab textmodels` (20 sentences, 3 passes): accuracy by 1 minus the word error rate of
    /// the plain-correction answers (>= 92 % is 5, >= 85 % 4, >= 75 % 3, >= 65 % 2), speed by the median seconds per sentence (<= 0.15 is 5, <= 0.3 4,
    /// <= 0.6 3, <= 2 2), both through Cue's own prompt and guard. Re-measure and update when a model is added.
    pub accuracy: u8,
    pub speed: u8,
    /// Private memory of the server with the model loaded, in MB.
    pub memory_mb: u32,
    /// Context window the server is started with, in tokens, shared by the task's prefix, the text and the answer. The request budget follows it.
    pub context: u32,
    pub repo: &'static str,
    pub revision: &'static str,
    pub file: ModelFile,
}

pub const MODELS: &[TextModel] = &[
    TextModel {
        id: "qwen2.5-1.5b",
        name: "Qwen2.5 1.5B",
        summary: "Most accurate, best all-round",
        languages: "29 languages",
        accuracy: 5,
        speed: 4,
        memory_mb: 1460,
        context: 4096,
        repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
        revision: "91cad51170dc346986eccefdc2dd33a9da36ead9",
        file: ModelFile {
            name: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
            size: 1_117_320_736,
            sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
        },
    },
    TextModel {
        id: "gemma-3-1b",
        name: "Gemma 3 1B",
        summary: "Broadest language coverage",
        languages: "140 languages",
        accuracy: 4,
        speed: 3,
        memory_mb: 1390,
        context: 4096,
        repo: "unsloth/gemma-3-1b-it-GGUF",
        revision: "f0b45be0aac41bd6a100a4b5734cad5f67255bfb",
        file: ModelFile {
            name: "gemma-3-1b-it-Q4_K_M.gguf",
            size: 806_058_272,
            sha256: "8270790f3ab69fdfe860b7b64008d9a19986d8df7e407bb018184caa08798ebd",
        },
    },
    TextModel {
        id: "lfm2-1.2b",
        name: "Liquid LFM2 1.2B",
        summary: "Fastest and lightest",
        languages: "8 languages",
        accuracy: 4,
        speed: 4,
        memory_mb: 1070,
        context: 4096,
        repo: "LiquidAI/LFM2-1.2B-GGUF",
        revision: "5399e76c648f4eb8c053feb1ab747277dea5bf8b",
        file: ModelFile {
            name: "LFM2-1.2B-Q4_K_M.gguf",
            size: 730_893_248,
            sha256: "55175400e3f509a9616227afeffd58d87e80b9f628a5d3d54ada884d85221fed",
        },
    },
];

/// llama.cpp b11559, Windows x64, Vulkan: it falls back to the CPU backends in the same archive when there is no usable GPU.
const SERVER: ModelFile = ModelFile {
    name: "llama-b11559-bin-win-vulkan-x64.zip",
    size: 32_084_119,
    sha256: "bf2fb2a7fc5d7669ccabbb6de3f6dd6b54058f213b32e731abc62b590f8566dc",
};
const SERVER_URL: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b11559/llama-b11559-bin-win-vulkan-x64.zip";
pub(crate) const SERVER_EXE: &str = "llama-server.exe";
/// Size of the unpacked server folder (52 files), for the free-disk check.
const SERVER_UNPACKED: u64 = 91_190_557;

/// Under the local data dir, next to the speech models, so "Remove all Cue data" deletes it too.
fn root(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_local_data_dir().ok()?.join("models").join("text"))
}

pub fn model_dir(app: &AppHandle, model: &TextModel) -> Option<PathBuf> {
    Some(root(app)?.join(model.id))
}

pub fn model_path(app: &AppHandle, model: &TextModel) -> Option<PathBuf> {
    Some(model_dir(app, model)?.join(model.file.name))
}

pub fn server_path(app: &AppHandle) -> Option<PathBuf> {
    Some(root(app)?.join("server").join(SERVER_EXE))
}

pub fn find(id: &str) -> Option<&'static TextModel> {
    MODELS.iter().find(|m| m.id == id)
}

fn server_installed(app: &AppHandle) -> bool {
    server_path(app).is_some_and(|p| p.exists())
}

/// A file only gets its final name after its hash was checked, so the right size means complete.
pub fn installed(app: &AppHandle, model: &TextModel) -> bool {
    let file = model_path(app, model).and_then(|p| std::fs::metadata(p).ok());
    file.is_some_and(|m| m.len() == model.file.size) && server_installed(app)
}

/// The model the Models page recommends: the lightest one on a weak PC.
pub fn recommended() -> &'static str {
    if crate::platform::weak_machine() {
        "lfm2-1.2b"
    } else {
        "qwen2.5-1.5b"
    }
}

/// The model the user chose if it is installed, otherwise the first installed one (like the speech engine).
pub fn current(app: &AppHandle) -> Option<&'static TextModel> {
    let chosen = crate::settings::get_str("text_model").and_then(|id| find(&id));
    chosen.filter(|m| installed(app, m)).or_else(|| MODELS.iter().find(|m| installed(app, m)))
}

/// Download size in bytes: the model, plus the server archive while it is not unpacked yet.
pub fn total_size(app: &AppHandle, model: &TextModel) -> u64 {
    model.file.size + if server_installed(app) { 0 } else { SERVER.size }
}

pub fn wanted(app: &AppHandle, model: &'static TextModel) -> Result<Vec<Wanted>, String> {
    let (root, dir) = (root(app), model_dir(app, model));
    let (Some(root), Some(dir)) = (root, dir) else { return Err("Cue has no folder to store models in.".into()) };
    let url = format!("https://huggingface.co/{}/resolve/{}/{}", model.repo, model.revision, model.file.name);
    let mut files = vec![Wanted::own(url, &model.file, dir)];
    if !server_installed(app) {
        let shared = Shared {
            skip_if: server_path(app),
            then: Some(std::sync::Arc::new({
                let app = app.clone();
                move |ctl| unpack(&app, ctl)
            })),
        };
        files.insert(
            0,
            Wanted { shared: Some(shared), unpacked: SERVER_UNPACKED, ..Wanted::own(SERVER_URL.into(), &SERVER, root.join("archive")) },
        );
    }
    Ok(files)
}

/// Puts the unpacked server in place: a cancel that arrived first removes the staging folder and installs nothing, otherwise `target` is replaced with `staging` by one rename. This is not the commit point, because the server is shared and useful alone while the model's own file is still to come; a cancel after it still cancels the job. A staging folder without the server is removed and refused.
pub(crate) fn publish_server(staging: &Path, target: &Path, ctl: &Ctl) -> Result<(), String> {
    if !staging.join(SERVER_EXE).exists() {
        let _ = std::fs::remove_dir_all(staging);
        return Err("Could not unpack the text engine. Try the download again.".into());
    }
    if ctl.is_cancelled() {
        let _ = std::fs::remove_dir_all(staging);
        return Err("Cancelled.".into());
    }
    let _ = std::fs::remove_dir_all(target);
    std::fs::rename(staging, target).map_err(|e| format!("Could not install the text engine: {e}"))
}

/// Unpacks the server archive into a staging folder, then renames the whole folder into place, so the server folder only ever exists complete. Deletes the archive afterwards.
/// Safe to repeat: when the server is installed already (another model's download got there first) it only tidies up.
pub fn unpack(app: &AppHandle, ctl: &Ctl) -> Result<(), String> {
    let root = root(app).ok_or("Cue has no folder to store models in.")?;
    let (archive, target, staging) = (root.join("archive").join(SERVER.name), root.join("server"), root.join("server.staging"));
    if !target.join(SERVER_EXE).exists() {
        let _ = std::fs::remove_dir_all(&staging);
        let unpacked = crate::platform::extract_zip(&archive, &staging, &|c| run_child(c, ctl));
        if let Err(e) = unpacked {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("Could not unpack the text engine: {e}"));
        }
        publish_server(&staging, &target, ctl)?;
    }
    let _ = std::fs::remove_dir_all(root.join("archive"));
    Ok(())
}

/// Removes a staging folder a quit or crash left behind.
pub fn recover(app: &AppHandle) {
    if let Some(root) = root(app) {
        let _ = std::fs::remove_dir_all(root.join("server.staging"));
    }
}

/// Deletes one model. The shared server goes with the last model, so nothing is left behind.
pub fn remove(app: &AppHandle, model: &TextModel) -> std::io::Result<()> {
    if let Some(dir) = model_dir(app, model) {
        match std::fs::remove_dir_all(dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    let any_left = MODELS.iter().any(|m| model_path(app, m).is_some_and(|p| p.exists()));
    let Some(root) = root(app).filter(|_| !any_left) else { return Ok(()) };
    for shared in ["server", "server.staging", "archive"] {
        match std::fs::remove_dir_all(root.join(shared)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    let _ = std::fs::remove_dir(root); // only succeeds when empty: another model's partial download stays
    Ok(())
}
