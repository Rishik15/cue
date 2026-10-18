# Third-party notices

Cue is built on open-source software. The main components and their licenses:

| Component                                                                                                                                 | License                   |
| ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------- |
| [Tauri](https://tauri.app/) and its plugins (store, autostart, single-instance, dialog)                                                   | MIT or Apache-2.0         |
| [rusqlite](https://github.com/rusqlite/rusqlite) with [SQLite](https://www.sqlite.org/) (snippets; SQLite is in the public domain)         | MIT / public domain       |
| [SolidJS](https://www.solidjs.com/), [Kobalte](https://kobalte.dev/), [Vite](https://vite.dev/), [Tailwind CSS](https://tailwindcss.com/) | MIT                       |
| [TypeScript](https://www.typescriptlang.org/)                                                                                             | Apache-2.0                |
| [cpal](https://github.com/RustAudio/cpal)                                                                                                 | Apache-2.0                |
| [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) with ONNX Runtime (speech engine), [ureq](https://github.com/algesten/ureq), [sha2](https://github.com/RustCrypto/hashes) | Apache-2.0 / MIT |
| [rubato](https://github.com/HEnquist/rubato), [serde](https://serde.rs/), [windows-rs](https://github.com/microsoft/windows-rs)           | MIT or Apache-2.0         |
| [handy-keys](https://github.com/handy-computer/handy-keys) (vendored with one patch, see `vendor/handy-keys/CUE_PATCH.md`)                | MIT                       |
| [Lucide](https://lucide.dev/) icons (interface icons and tray menu icons)                                                                 | ISC                       |
| [Inter](https://rsms.me/inter/) variable font and static Regular, Medium, SemiBold, Italic subsets (bundled)                                      | SIL Open Font License 1.1 |

Notices required by those licenses:

- handy-keys: Copyright (c) 2026 handy-computer. MIT License; the full text is in `vendor/handy-keys/LICENSE`.
- Lucide: Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022 as part of Feather (MIT). All other copyright (c) for Lucide are held by Lucide Contributors 2022. Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies. The software is provided "as is" without warranty of any kind.
- Inter: Copyright (c) 2016 The Inter Project Authors (https://github.com/rsms/inter). This Font Software is licensed under the SIL Open Font License, Version 1.1, available at https://openfontlicense.org. The font is bundled unmodified and is not sold by itself.
- Speech models (downloaded on request, not bundled), used unmodified: NVIDIA Parakeet TDT 0.6B v2 and v3 and NVIDIA Canary 180M Flash, licensed CC-BY-4.0 (https://creativecommons.org/licenses/by/4.0/); NVIDIA Nemotron Speech Streaming 0.6B and Nemotron 3.5 ASR Streaming under the NVIDIA Open Model License Agreement; OpenAI Whisper Small (MIT); Silero VAD by Snakers4 (MIT). The int8 ONNX conversions are by the sherpa-onnx project on Hugging Face (csukuangfj).
- Text models and runtime (downloaded on request, not bundled), used unmodified: Qwen2.5 1.5B Instruct (Apache-2.0), Gemma 3 1B (Google's Gemma Terms of Use), Liquid LFM2 1.2B (Liquid AI's LFM Open License v1.0), and the llama.cpp server (MIT).
- Dictionary pack (downloaded on request, not bundled): English Wiktionary (https://en.wiktionary.org), extracted by kaikki.org (https://kaikki.org), licensed CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/). Cue trims it to meanings, one example per meaning and synonyms and repacks it as one SQLite file, which carries the same licence.
- MIT and Apache-2.0 components: each is used under its own license. Their copyright notices are kept in their source packages and are available from the package registries.

Design and technique references. No code was copied from these projects unless stated above:

- [Handy](https://github.com/cjpais/Handy) (MIT) informed the shortcut handling, audio capture, clipboard-paste and release-grace ideas, which were reimplemented.
- [Pouchy](https://github.com/editorrylix/Pouchy) informed the tray menu's behaviour (an app-drawn popup that never steals focus). Its code is not used.
- Inter: Copyright (c) 2016 The Inter Project Authors. SIL Open Font License 1.1; the full text is in `src-tauri/assets/fonts/OFL.txt`.

Planned components, used under their own licenses when added: Tesseract (Apache-2.0) and a file-system watcher.
