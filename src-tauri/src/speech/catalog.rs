//! Purpose: The speech models Cue offers, pinned to exact Hugging Face revisions with sizes and SHA-256, and where they live on disk.
//! Contents: Model / ModelFile — one entry (with the short facts the Models page shows); MODELS — the curated list (Parakeet v2 and v3, Canary Lite, Whisper Small, all int8); DEFAULT — the pick for new installs;
//! VAD — the Silero voice detector every model uses (one shared 1.8 MB file); find / dir / vad_path / installed — lookups. Kept to a few proven models instead of a long list: each is tested to run on every laptop Cue supports.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use super::protocol::Family;

pub struct ModelFile {
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub languages: &'static str,
    /// 1 to 5 from `scripts/winsync models` (73 LibriSpeech clips): accuracy by word error rate (<= 3 % is 5, <= 4 % 4, <= 5 % 3, <= 8 % 2),
    /// speed by multiple of real time (>= 18x is 5, >= 12x 4, >= 8x 3, >= 4x 2). Re-measure and update when a model is added.
    pub accuracy: u8,
    pub speed: u8,
    /// Private memory of the worker after those clips, in MB.
    pub memory_mb: u32,
    pub recommended: bool,
    pub family: Family,
    pub repo: &'static str,
    pub revision: &'static str,
    pub files: &'static [ModelFile],
}

pub const DEFAULT: &str = "parakeet-v2";

/// Silero voice activity detector, shared by all models: cuts the recording at pauses and drops silence.
pub const VAD: ModelFile =
    ModelFile { name: "silero_vad.onnx", size: 1_807_522, sha256: "a35ebf52fd3ce5f1469b2a36158dba761bc47b973ea3382b3186ca15b1f5af28" };
pub const VAD_URL: &str = "https://huggingface.co/csukuangfj/vad/resolve/fba88cd2e921609e7675c3aaf51e0b9b295da4bc/silero_vad.onnx";

pub const MODELS: &[Model] = &[
    Model {
        id: "parakeet-v2",
        name: "Parakeet v2",
        summary: "Fast, accurate English dictation.",
        languages: "English",
        accuracy: 5,
        speed: 5,
        memory_mb: 1010,
        recommended: true,
        family: Family::Transducer,
        repo: "csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8",
        revision: "1ab9323565ddb038682214b292f588070a538ce2",
        files: &[
            ModelFile { name: "tokens.txt", size: 9_384, sha256: "ec182b70dd42113aff6c5372c75cac58c952443eb22322f57bbd7f53977d497d" },
            ModelFile {
                name: "joiner.int8.onnx",
                size: 1_739_080,
                sha256: "7946164367946e7f9f29a122407c3252b680dbae9a51343eb2488d057c3c43d2",
            },
            ModelFile {
                name: "decoder.int8.onnx",
                size: 7_257_753,
                sha256: "b6bb64963457237b900e496ee9994b59294526439fbcc1fecf705b31a15c6b4e",
            },
            ModelFile {
                name: "encoder.int8.onnx",
                size: 652_184_296,
                sha256: "a32b12d17bbbc309d0686fbbcc2987b5e9b8333a7da83fa6b089f0a2acd651ab",
            },
        ],
    },
    Model {
        id: "parakeet-v3",
        name: "Parakeet v3",
        summary: "Detects the language automatically.",
        languages: "25 languages",
        accuracy: 3,
        speed: 5,
        memory_mb: 1020,
        recommended: false,
        family: Family::Transducer,
        repo: "csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
        revision: "2bda32ec70b097a55adaa07d9a7173915b43cc78",
        files: &[
            ModelFile { name: "tokens.txt", size: 93_939, sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d" },
            ModelFile {
                name: "joiner.int8.onnx",
                size: 6_355_277,
                sha256: "3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3",
            },
            ModelFile {
                name: "decoder.int8.onnx",
                size: 11_845_275,
                sha256: "179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e",
            },
            ModelFile {
                name: "encoder.int8.onnx",
                size: 652_184_281,
                sha256: "acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247",
            },
        ],
    },
    Model {
        id: "canary-lite",
        name: "Canary Lite",
        summary: "Light on memory, for older laptops.",
        languages: "English",
        accuracy: 4,
        speed: 4,
        memory_mb: 630,
        recommended: false,
        family: Family::Canary,
        repo: "csukuangfj/sherpa-onnx-nemo-canary-180m-flash-en-es-de-fr-int8",
        revision: "9077164e0d3dd1d5353743e89ceaa1d3a770838c",
        files: &[
            ModelFile { name: "tokens.txt", size: 53_555, sha256: "2dae6fc7815f9640645e0c765522b278ee0cef49b482d91f6913e334628d3e77" },
            ModelFile {
                name: "decoder.int8.onnx",
                size: 74_437_848,
                sha256: "e41a2ab9c0c2fe81a1e8ade5a45fb02a74bc4db7d1f91b89a54a25e2cf79cba2",
            },
            ModelFile {
                name: "encoder.int8.onnx",
                size: 132_678_643,
                sha256: "7a75b4e2a5857a6dcc0819503bbe3fad66943db4a3ccf21d3f27c633667d303f",
            },
        ],
    },
    Model {
        id: "whisper-small",
        name: "Whisper Small",
        summary: "Widest language coverage, slower.",
        languages: "99 languages",
        accuracy: 2,
        speed: 2,
        memory_mb: 1590,
        recommended: false,
        family: Family::Whisper,
        repo: "csukuangfj/sherpa-onnx-whisper-small",
        revision: "8f3c18b358db4d1f2fc1eae49d75cd20989e4309",
        files: &[
            ModelFile {
                name: "small-tokens.txt",
                size: 816_730,
                sha256: "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126",
            },
            ModelFile {
                name: "small-decoder.int8.onnx",
                size: 262_226_114,
                sha256: "acad50b5c782696e91b55914cc5ab4f756f1532f76e22aa6fc615f39fb69a8ee",
            },
            ModelFile {
                name: "small-encoder.int8.onnx",
                size: 112_442_483,
                sha256: "4cbe7b22fa9026b843b60a68640c747de05bafb1a11b57edc0e66c232d9f33a9",
            },
        ],
    },
];

impl Model {
    /// Everything a download fetches, including the shared voice detector.
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum::<u64>() + VAD.size
    }

    pub fn url(&self, file: &ModelFile) -> String {
        format!("https://huggingface.co/{}/resolve/{}/{}", self.repo, self.revision, file.name)
    }
}

pub fn find(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

/// Under the local data dir, which "Remove all Cue data" already deletes.
pub fn models_dir(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_local_data_dir().ok()?.join("models"))
}

pub fn dir(app: &AppHandle, model: &Model) -> Option<PathBuf> {
    Some(models_dir(app)?.join(model.id))
}

pub fn vad_path(app: &AppHandle) -> Option<PathBuf> {
    Some(models_dir(app)?.join(VAD.name))
}

fn complete(dir: &std::path::Path, file: &ModelFile) -> bool {
    std::fs::metadata(dir.join(file.name)).is_ok_and(|m| m.len() == file.size)
}

/// A file only gets its final name after its hash was checked, so existence with the right size means complete.
pub fn installed(app: &AppHandle, model: &Model) -> bool {
    let (Some(dir), Some(root)) = (dir(app, model), models_dir(app)) else { return false };
    complete(&root, &VAD) && model.files.iter().all(|f| complete(&dir, f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        assert!(find(DEFAULT).is_some());
        for m in MODELS {
            assert_eq!(m.revision.len(), 40);
            assert!(m.files.iter().all(|f| f.sha256.len() == 64 && f.size > 0));
            assert_eq!(VAD.sha256.len(), 64);
            assert!((1..=5).contains(&m.accuracy) && (1..=5).contains(&m.speed));
            assert!(m.files.iter().any(|f| f.name.ends_with("tokens.txt")));
        }
    }
}
