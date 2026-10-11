//! Purpose: The text model Cue runs for Correct and Rewrite, and the llama.cpp server that runs it: pinned URLs, sizes and SHA-256, where they live, how the server archive is unpacked.
//! Contents: ID / NAME / SUMMARY / MEMORY_MB — what the Models page shows; wanted — the files a download fetches; installed — both the model and the server are on disk; model_path / server_path — lookups; unpack — extract the server archive.
//! One model on purpose: Qwen2.5 1.5B Instruct (Apache-2.0, 1.1 GB at Q4_K_M) fixes and rewrites short text well on a plain CPU. A list can come when a second model earns its place.

use std::path::PathBuf;
use std::process::Command;

use tauri::{AppHandle, Manager};

use crate::speech::catalog::ModelFile;
use crate::speech::download::Wanted;

pub const ID: &str = "qwen2.5-1.5b";
pub const NAME: &str = "Qwen2.5 1.5B";
pub const SUMMARY: &str = "Corrects and rewrites text on your device.";
pub const MEMORY_MB: u32 = 1700;

const MODEL: ModelFile = ModelFile {
    name: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    size: 1_117_320_736,
    sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
};
const MODEL_URL: &str =
    "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/91cad51170dc346986eccefdc2dd33a9da36ead9/qwen2.5-1.5b-instruct-q4_k_m.gguf";

/// llama.cpp b11559, Windows x64, Vulkan: it falls back to the CPU backends in the same archive when there is no usable GPU.
const SERVER: ModelFile = ModelFile {
    name: "llama-b11559-bin-win-vulkan-x64.zip",
    size: 32_084_119,
    sha256: "bf2fb2a7fc5d7669ccabbb6de3f6dd6b54058f213b32e731abc62b590f8566dc",
};
const SERVER_URL: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b11559/llama-b11559-bin-win-vulkan-x64.zip";
const SERVER_EXE: &str = "llama-server.exe";

/// Under the local data dir, next to the speech models, so "Remove all Cue data" deletes it too.
pub fn dir(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_local_data_dir().ok()?.join("models").join("text"))
}

pub fn model_path(app: &AppHandle) -> Option<PathBuf> {
    Some(dir(app)?.join(MODEL.name))
}

pub fn server_path(app: &AppHandle) -> Option<PathBuf> {
    Some(dir(app)?.join("server").join(SERVER_EXE))
}

/// A file only gets its final name after its hash was checked, so the right size means complete.
pub fn installed(app: &AppHandle) -> bool {
    let model = model_path(app).and_then(|p| std::fs::metadata(p).ok());
    model.is_some_and(|m| m.len() == MODEL.size) && server_path(app).is_some_and(|p| p.exists())
}

/// Download size in bytes: the model, plus the server archive while it is not unpacked yet.
pub fn total_size(app: &AppHandle) -> u64 {
    MODEL.size + if server_path(app).is_some_and(|p| p.exists()) { 0 } else { SERVER.size }
}

pub fn wanted(app: &AppHandle) -> Result<Vec<Wanted>, String> {
    let dir = dir(app).ok_or("Cue has no folder to store models in.")?;
    let mut files = vec![Wanted { url: MODEL_URL.into(), file: &MODEL, dir: dir.clone() }];
    if !server_path(app).is_some_and(|p| p.exists()) {
        files.push(Wanted { url: SERVER_URL.into(), file: &SERVER, dir: dir.join("archive") });
    }
    Ok(files)
}

/// Unpacks the server archive with the `tar` that ships with Windows 10 and later (it reads zip), then deletes the archive.
pub fn unpack(app: &AppHandle) -> Result<(), String> {
    let dir = dir(app).ok_or("Cue has no folder to store models in.")?;
    let (archive, target) = (dir.join("archive").join(SERVER.name), dir.join("server"));
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
    let _ = std::fs::remove_dir_all(dir.join("archive"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_are_well_formed() {
        for f in [&MODEL, &SERVER] {
            assert_eq!(f.sha256.len(), 64);
            assert!(f.size > 0);
        }
        assert!(MODEL_URL.contains("91cad51170dc346986eccefdc2dd33a9da36ead9"));
    }
}
