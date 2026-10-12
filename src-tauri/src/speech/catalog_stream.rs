//! Purpose: The streaming speech models (NVIDIA Nemotron, int8, as packaged by sherpa-onnx) used for long dictation, pinned to exact release
//! archives with size and SHA-256, and the unpacking of an archive into the model folder.
//! Ratings: memory and speed measured on a 10 minute stream (799 MB flat, about 4.6x real time on 2 threads); accuracy from the model card (LibriSpeech
//! clean 2.5 %, 7 % on the harder sets; multilingual English 7.9 % on FLEURS), not on our own clips.
//! Contents: STREAMING — English and multilingual, both with 560 ms chunks (the best balance of lag and accuracy on the model card);
//! unpack — extract the archive with Windows' own `tar` and check the four files by size.

use std::path::PathBuf;
use std::process::Command;

use tauri::AppHandle;

use super::catalog::{dir, Archive, Model, ModelFile};
use super::protocol::Family;

pub const STREAMING: &[Model] = &[
    Model {
        id: "nemotron-en",
        name: "Nemotron Streaming",
        summary: "Built for long sessions: keeps up for hours.",
        languages: "English",
        accuracy: 4,
        speed: 2,
        memory_mb: 800,
        recommended: false,
        family: Family::Streaming,
        repo: "",
        revision: "",
        files: &[
            ModelFile { name: "tokens.txt", size: 8_952, sha256: "dc0b4584ab2e4ddbf888425c076c61b736e7356a015250db7d307e6f1a8188ff" },
            ModelFile { name: "joiner.int8.onnx", size: 1_735_862, sha256: "a35eac38a22ebceb04d230ed7afe0d68f446ba6914a036b97f14fece95967e23" },
            ModelFile { name: "decoder.int8.onnx", size: 7_257_753, sha256: "0be9702c2f427a2b6bb241d298e0d3836a558de1f5b9fd3018f1cce6e2b3fa98" },
            ModelFile { name: "encoder.int8.onnx", size: 652_916_849, sha256: "7d932213491ad355c6e5576705dc3494731a52af87d7a1b954559340147909d8" },
        ],
        archive: Some(Archive {
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25.tar.bz2",
            file: ModelFile {
                name: "nemotron-en.tar.bz2",
                size: 463_945_051,
                sha256: "78e2b79fcf7271553a74402a76b771b09ea40117a39566a79f52235b23db6358",
            },
        }),
    },
    Model {
        id: "nemotron-multi",
        name: "Nemotron Streaming Multilingual",
        summary: "Long sessions in many languages.",
        languages: "32 languages",
        accuracy: 3,
        speed: 2,
        memory_mb: 810,
        recommended: false,
        family: Family::StreamingMulti,
        repo: "",
        revision: "",
        files: &[
            ModelFile { name: "tokens.txt", size: 131_440, sha256: "729cc103155bafa785f9cd45746cd41cabe97eab7182fc04d594129587958f8a" },
            ModelFile { name: "joiner.int8.onnx", size: 9_504_438, sha256: "4101c7c679a0bc30483794b27a059e34e79232aa2068d78d51231a22c8b0d7ce" },
            ModelFile { name: "decoder.int8.onnx", size: 14_978_075, sha256: "19f9c98fc6d0a2c33a65a43b36fdb2e914c26c0aa9764be3aebc502a1e982fb0" },
            ModelFile { name: "encoder.int8.onnx", size: 657_601_403, sha256: "012e9321373af99021415e0b0eb3ec827b4be3153be6f30d9b448fe65e896e68" },
        ],
        archive: Some(Archive {
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-560ms-int8-2026-06-11.tar.bz2",
            file: ModelFile {
                name: "nemotron-multi.tar.bz2",
                size: 475_271_763,
                sha256: "c6bf5e0df765f9d5b43bc9e0536d4b4b3e7d40bdf5ecf13e45f134c51c05ae3a",
            },
        }),
    },
];

/// Unpacks the downloaded archive into the model folder (its top folder stripped) with the `tar` that ships with Windows 10 and later, checks
/// the four files by size, and deletes the archive.
pub fn unpack(app: &AppHandle, model: &'static Model) -> Result<(), String> {
    let (Some(target), Some(archive)) = (dir(app, model), model.archive.as_ref()) else {
        return Err("Cue has no folder to store models in.".into());
    };
    let file = target.join(archive.file.name);
    // Windows' own tar by its full path, so a program called `tar` earlier on the PATH cannot stand in for it.
    let system = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let mut tar = Command::new(system.join("System32").join("tar.exe"));
    tar.arg("-xf").arg(&file).arg("-C").arg(&target).arg("--strip-components=1");
    crate::platform::hide_console(&mut tar);
    let done = tar.status().map_err(|e| format!("Could not unpack the model: {e}"))?;
    let whole = model.files.iter().all(|f| std::fs::metadata(target.join(f.name)).is_ok_and(|m| m.len() == f.size));
    if !done.success() || !whole {
        return Err("Could not unpack the model. Try the download again.".into());
    }
    let _ = std::fs::remove_file(file);
    Ok(())
}
