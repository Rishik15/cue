//! Purpose: The text models Cue offers for Correct and Rewrite, and the llama.cpp server that runs them: pinned URLs, sizes and SHA-256, where they live, how the server archive is unpacked.
//! Contents: TextModel / MODELS — the curated list with the facts the Models page shows; find / current — lookups (the chosen model if installed, else any installed one); wanted — the files a download fetches;
//! installed — model and server are on disk; model_path / server_path / model_dir — locations; unpack — extract the server archive; remove — delete one model (and the server with the last one).
//! The server is shared by all models: it is downloaded once with the first model.

use std::path::PathBuf;
use std::process::Command;

use tauri::{AppHandle, Manager};

use crate::speech::catalog::ModelFile;
use crate::speech::download::Wanted;

pub struct TextModel {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub languages: &'static str,
    /// 1 to 5 from `scripts/winsync textmodels` (20 sentences, 3 passes, see `docs/M2_build_log.md`): accuracy by 1 minus the word error rate of
    /// Correct Text answers (>= 92 % is 5, >= 85 % 4, >= 75 % 3, >= 65 % 2), speed by the median seconds per sentence (<= 0.15 is 5, <= 0.3 4,
    /// <= 0.6 3, <= 2 2). Re-measure and update when a model is added.
    pub accuracy: u8,
    pub speed: u8,
    /// Private memory of the server with the model loaded, in MB.
    pub memory_mb: u32,
    pub recommended: bool,
    /// Reasoning models write a long `<think>` passage first, so they get a larger generation budget and the passage is dropped from the answer.
    pub thinks: bool,
    pub repo: &'static str,
    pub revision: &'static str,
    pub file: ModelFile,
}

pub const MODELS: &[TextModel] = &[
    TextModel {
        id: "qwen2.5-1.5b",
        name: "Qwen2.5 1.5B",
        summary: "Most accurate corrections. The best all-round pick.",
        languages: "29 languages",
        accuracy: 5,
        speed: 3,
        memory_mb: 1460,
        recommended: true,
        thinks: false,
        repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
        revision: "91cad51170dc346986eccefdc2dd33a9da36ead9",
        file: ModelFile {
            name: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
            size: 1_117_320_736,
            sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
        },
    },
    TextModel {
        id: "llama-3.2-1b",
        name: "Llama 3.2 1B",
        summary: "Weak at corrections in tests. Better kept for experiments.",
        languages: "8 languages",
        accuracy: 1,
        speed: 4,
        memory_mb: 1350,
        recommended: false,
        thinks: false,
        repo: "bartowski/Llama-3.2-1B-Instruct-GGUF",
        revision: "067b946cf014b7c697f3654f621d577a3e3afd1c",
        file: ModelFile {
            name: "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
            size: 807_694_464,
            sha256: "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83",
        },
    },
    TextModel {
        id: "gemma-3-1b",
        name: "Gemma 3 1B",
        summary: "Close behind Qwen and covers the most languages.",
        languages: "140 languages",
        accuracy: 4,
        speed: 3,
        memory_mb: 1390,
        recommended: false,
        thinks: false,
        repo: "unsloth/gemma-3-1b-it-GGUF",
        revision: "f0b45be0aac41bd6a100a4b5734cad5f67255bfb",
        file: ModelFile {
            name: "gemma-3-1b-it-Q4_K_M.gguf",
            size: 806_058_272,
            sha256: "8270790f3ab69fdfe860b7b64008d9a19986d8df7e407bb018184caa08798ebd",
        },
    },
    TextModel {
        id: "deepseek-r1-1.5b",
        name: "DeepSeek R1 Distill 1.5B",
        summary: "Thinks before it answers. Far too slow for quick fixes.",
        languages: "English, Chinese",
        accuracy: 1,
        speed: 1,
        memory_mb: 1970,
        recommended: false,
        thinks: true,
        repo: "bartowski/DeepSeek-R1-Distill-Qwen-1.5B-GGUF",
        revision: "9cc28b17e86fa2415fcb070f8ee5ec27c965aa61",
        file: ModelFile {
            name: "DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf",
            size: 1_117_320_800,
            sha256: "1741e5b2d062b07acf048bf0d2c514dadf2a48f94e2b4aa0cfe069af3838ee2f",
        },
    },
    TextModel {
        id: "lfm2-350m",
        name: "Liquid LFM2 350M",
        summary: "Instant and tiny, for simple mistakes only.",
        languages: "8 languages",
        accuracy: 2,
        speed: 5,
        memory_mb: 495,
        recommended: false,
        thinks: false,
        repo: "LiquidAI/LFM2-350M-GGUF",
        revision: "8fdc9d526b7ed346b19257551b05816c7912ecc2",
        file: ModelFile {
            name: "LFM2-350M-Q4_K_M.gguf",
            size: 229_309_376,
            sha256: "a4d000c7064bd3b2e42c6845836286a899a4e79cf1791da1a6797b58d575957d",
        },
    },
    TextModel {
        id: "lfm2-1.2b",
        name: "Liquid LFM2 1.2B",
        summary: "Fast and light on memory. Good for quick edits.",
        languages: "8 languages",
        accuracy: 3,
        speed: 4,
        memory_mb: 1070,
        recommended: false,
        thinks: false,
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
const SERVER_EXE: &str = "llama-server.exe";

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

/// The model the user chose if it is installed, otherwise the first installed one (like the speech engine).
pub fn current(app: &AppHandle) -> Option<&'static TextModel> {
    let chosen = crate::settings::get_str(app, "text_model").and_then(|id| find(&id));
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
    let mut files = vec![Wanted { url, file: &model.file, dir }];
    if !server_installed(app) {
        files.push(Wanted { url: SERVER_URL.into(), file: &SERVER, dir: root.join("archive") });
    }
    Ok(files)
}

/// Unpacks the server archive with the `tar` that ships with Windows 10 and later (it reads zip), then deletes the archive.
pub fn unpack(app: &AppHandle) -> Result<(), String> {
    let root = root(app).ok_or("Cue has no folder to store models in.")?;
    let (archive, target) = (root.join("archive").join(SERVER.name), root.join("server"));
    if !archive.exists() {
        return Ok(()); // already unpacked by an earlier download
    }
    std::fs::create_dir_all(&target).map_err(|e| format!("Could not create the server folder: {e}"))?;
    let mut tar = Command::new("tar");
    tar.arg("-xf").arg(&archive).arg("-C").arg(&target);
    crate::platform::hide_console(&mut tar);
    let done = tar.status().map_err(|e| format!("Could not unpack the text engine: {e}"))?;
    if !done.success() || !target.join(SERVER_EXE).exists() {
        return Err("Could not unpack the text engine. Try the download again.".into());
    }
    let _ = std::fs::remove_dir_all(root.join("archive"));
    Ok(())
}

/// Deletes one model. The shared server goes with the last model, so nothing is left behind.
pub fn remove(app: &AppHandle, model: &TextModel) -> std::io::Result<()> {
    if let Some(dir) = model_dir(app, model) {
        std::fs::remove_dir_all(dir)?;
    }
    let any_left = MODELS.iter().any(|m| model_path(app, m).is_some_and(|p| p.exists()));
    match root(app).filter(|_| !any_left) {
        Some(root) => std::fs::remove_dir_all(root),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        for m in MODELS {
            assert_eq!(m.revision.len(), 40, "{}", m.id);
            assert_eq!(m.file.sha256.len(), 64, "{}", m.id);
            assert!(m.file.size > 0);
        }
        let mut ids: Vec<_> = MODELS.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), MODELS.len(), "ids are unique");
        assert_eq!(SERVER.sha256.len(), 64);
    }
}
