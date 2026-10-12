<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

**A local desktop utility for dictating, capturing, editing and reusing text.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

Cue sits quietly in your system tray and helps you finish small writing and copy/paste tasks inside the apps you already use: speak instead of typing, fix a sentence, grab text from the screen, insert text you use all the time. Everything runs on your device. Text, audio and screenshots are never sent anywhere.

**Status: early development, version 0.1.0.** The app shell, dictation and text rules are done; Windows is the only supported platform. Speech is recognised on your device by a downloadable model and typed where your cursor is, and rules tidy dictated or selected text with exact transformations. Correct and Rewrite run on a small local language model that you download on the Models page. See [what works now](#what-works-now) and the [roadmap](#roadmap).

## The idea

Most dictation and writing helpers are heavy, always-on and cloud-backed. Cue is the opposite, built on a few principles:

- **Local processing.** Text, audio and captured images stay on the device. The network is used only for explicit model downloads and app updates.
- **Direct interaction.** A shortcut gets you to a useful action with almost no navigation, in the app you are already in.
- **Quiet at rest.** In the tray Cue uses about 3.5 MB of private memory and no measurable CPU. The speech engine runs in its own process that exists only while you dictate, and the settings window is created when opened and destroyed when closed.
- **Preserve intent.** Edits keep your meaning. Exact transformations (case, whitespace, links) are ordinary code, not a language model.
- **One workflow.** Dictation, edits, screen text and saved snippets share the same preview, clipboard and insertion machinery.

## What works now

- **Tray app.** Single instance, optional launch at login. Right-click the tray icon for a short menu (Open Cue, Settings, Quit); left click opens Settings. The menu is drawn natively, with no web view, and costs nothing while closed.
- **Dictation shortcut.** Hold to talk, tap to toggle, or both (a quick tap toggles, a long press records while held). Default: Left Ctrl + Space. Choose your own, including Win combinations, left/right-specific modifiers, modifier-only keys and F-keys. Escape cancels.
- **Speech to text.** Parakeet, Canary and Whisper models run through sherpa-onnx in a separate worker process that starts when you press the shortcut and exits after the idle time you choose, so a loaded model costs memory only while it is useful and an engine crash cannot take Cue down. Speech is recognised sentence by sentence while you talk, so the last words are ready almost as soon as you stop. Measured on 73 clean English clips (`scripts/winsync models`): Parakeet v2 2.9 % word error rate at about 19x real time, Canary Lite 3.7 % at 13x, Parakeet v3 4.3 % at 19x, Whisper Small 7.7 % at 4x.
- **Models page.** Four models with measured accuracy, speed and memory, resumable checksum-verified downloads, and one-click switching. Nothing is bundled; everything runs offline once downloaded.
- **Live caption.** A native pill in your theme: sound bars while it listens (grey until the microphone is ready), then the text typed out as sentences are recognised, faster when more is waiting. The pill stays until your text is in place.
- **Microphone capture.** Pick a device, how long the mic stays open, the maximum length and the shortest recording that counts. A microphone that stops working is reopened on the next recording.
- **Text insertion.** Text is typed into the focused app; long text is pasted through the clipboard and your previous clipboard (including images and files) is put back. If you switch windows while Cue is working, or the target app runs as administrator, the text goes to the clipboard and Cue says so instead of typing into the wrong place. Newlines are never typed as Enter, so dictation cannot send a chat message.
- **Output options.** Insert in the active app or copy to the clipboard; Context-Aware Paste reads the few characters before the cursor and adds a leading space or fits capitalization; Auto Submit (off by default) presses Enter or Ctrl+Enter after the text is inserted.
- **Settings search.** The search box in the sidebar finds the actual settings, not just pages: it highlights the matches, and opening one scrolls to its row and flashes it.
- **Statistics and mute.** The Dictation page shows words per minute, typing time saved and total words, with an activity heatmap for the last year (counts only, kept in memory for now, never your text). "Mute Audio While Recording" silences your speakers while you dictate and restores them afterwards, even after a crash.
- **Vocabulary.** Names and terms you want spelled your way, replacements for phrases (`my email -> me@example.com`), and filler-word removal.
- **Rules.** A rule is a trigger, optional conditions (contains, starts or ends with, pattern, length, looks like a link, email, JSON or list, limited to chosen programs), an ordered list of actions, and where the result goes. Most actions are exact code, no model: clean whitespace, join broken lines from a PDF, change case, clean tracking parameters from links, extract emails or links. Four actions use a local text model (three to choose from on the Models page (Qwen2.5 1.5B, Gemma 3 1B, Liquid LFM2 1.2B), rated by measured accuracy and speed; run by llama.cpp with Vulkan or the CPU): Correct Text, Make Shorter, Make Formal and Make Friendly. The model's server starts when a rule needs it and stops after the idle time you chose, so it costs nothing at rest. Triggers: a finished dictation, or the selection shortcut (default Left Ctrl + Left Alt + Space), which transforms the text you have selected in any app and puts the result back. The result is typed in place, copied, or shown in a preview window to apply (Enter), copy (Ctrl+Enter) or cancel (Esc). Click a rule to edit it; its Delete Rule button is in the editor. The Rules page tests a draft live as you type a sample text and keeps a short history of recent runs (rule names only, never your text).
- **Settings.** A fast, frameless window with a sidebar search over the actual settings, back and forward arrows, smooth wheel scrolling, and six theme families (each with light and dark), a keyboard-friendly shortcut recorder for both shortcuts and a Remove All Cue Data action.

Not yet working: the command bar, rich text to Markdown, screen text capture, snippets, and macOS.

## Security

Everything runs on your device and Cue sends nothing anywhere. Prompt injection, the local model server, typed text, downloads and the settings window are covered in [SECURITY.md](SECURITY.md), which also says what Cue does not defend against.

## Roadmap

| Milestone                  | Goal                                                                                                                                                                                                       |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **M0 · App shell** (done)  | Tray, hotkey, mic capture, overlay, insertion, settings and design system, benchmarks.                                                                                                                     |
| **M1 · Foundation** (done) | Action registry, exact text tools, rule engine with a Rules page, selection shortcut and result preview. Left for later: the command bar (with M2) and rich text to Markdown (with M3's clipboard formats). |
| **M2 · Voice and editing** | Dictation, models, live caption and vocabulary are done. Correct and Rewrite are done (local llama.cpp). Next: the command bar, optional transcript cleanup, spoken edit commands. |
| **M3 · First release**     | Grab text from the screen (OCR), saved snippets with import and export (adds SQLite), rich text to Markdown, macOS support, full compatibility testing across apps.                                         |
| **M4 · Extensions**        | Opt-in clipboard history, folder rules, read aloud, snippet expansion, converters (units, dates, JSON, CSV), summaries and translation.                                                                   |

Out of scope: general chat, long-form drafting, meeting transcription, autonomous control of your computer, and cloud inference.

## Install

There is no release yet. Build it from source (below). The build produces a small per-user Windows installer: no administrator rights, no extra runtimes, and an uninstaller that can also remove your data.

## Build from source

Requirements: Rust (stable, MSVC toolchain), Node.js 24 or newer, pnpm (`corepack enable`), the Visual Studio Build Tools C++ workload, and the WebView2 runtime (included with current Windows).

```sh
pnpm install
pnpm tauri dev        # run with hot reload
pnpm tauri build      # build the installer
```

Run the checks the same way CI does (on WSL, `scripts/clippy-wsl` runs the Rust lint for Windows without leaving Linux):

```sh
pnpm check
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Performance numbers come from `scripts/bench.ps1` (needs a release build) and are gated in CI against `bench/baseline.json`.

## How it is built

Tauri 2 with a Rust backend and a SolidJS + TypeScript + Tailwind interface. The settings window is created when you open it and destroyed when you close it; the tray menu and recording overlay are drawn natively in Rust, so the process in the tray stays tiny.

```text
src/                 SolidJS interface: pages, UI kit (src/ui), settings and theme state
src-tauri/src/       Rust backend
  audio/             microphone capture, streaming resampler
  actions/           exact text actions (whitespace, case, links, extraction)
  rules/             rule file, conditions, engine, recent runs
  dictation/         hotkey modes, start/stop flow, transcript cleanup, speaker mute, context-aware paste
  stats.rs           dictation statistics (in memory)
  selection.rs       selection shortcut flow; preview.rs the preview window
  speech/            worker process, model catalog and downloads
  shortcut/          global shortcut engine
  platform/          every OS call: insertion, clipboard, overlay, tray popup (Windows)
  tray/              tray icon and menu content
vendor/handy-keys/   keyboard hook library, patched to sleep instead of polling and to keep a blocked hotkey key blocked until released
scripts/             benchmark and check scripts (`speech-smoke.ps1`: worker lifecycle, `selection-check.ps1`, `leak-check.ps1`, `clippy-wsl` for the Rust lint from WSL)
src-tauri/tests/     end-to-end speech worker tests and the model benchmark
```

## Privacy

Cue does not collect telemetry and does not send your text, audio or screenshots anywhere. Logs omit user content. Network access is limited to explicit model downloads and update checks (not yet implemented). Settings live in your user data directory, and Settings → About → Remove All Cue Data deletes them.

## License

Cue is released under the [MIT License](LICENSE).

## Third-party notices

Cue is built on open-source software. The main components and their licenses:

| Component                                                                                                                                 | License                   |
| ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------- |
| [Tauri](https://tauri.app/) and its plugins (store, autostart, single-instance)                                                           | MIT or Apache-2.0         |
| [SolidJS](https://www.solidjs.com/), [Kobalte](https://kobalte.dev/), [Vite](https://vite.dev/), [Tailwind CSS](https://tailwindcss.com/) | MIT                       |
| [TypeScript](https://www.typescriptlang.org/)                                                                                             | Apache-2.0                |
| [cpal](https://github.com/RustAudio/cpal)                                                                                                 | Apache-2.0                |
| [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) with ONNX Runtime (speech engine), [ureq](https://github.com/algesten/ureq), [sha2](https://github.com/RustCrypto/hashes) | Apache-2.0 / MIT |
| [rubato](https://github.com/HEnquist/rubato), [serde](https://serde.rs/), [windows-rs](https://github.com/microsoft/windows-rs)           | MIT or Apache-2.0         |
| [handy-keys](https://github.com/handy-computer/handy-keys) (vendored with one patch, see `vendor/handy-keys/CUE_PATCH.md`)                | MIT                       |
| [Lucide](https://lucide.dev/) icons (interface icons and tray menu icons)                                                                 | ISC                       |
| [Inter](https://rsms.me/inter/) variable font (bundled)                                                                                   | SIL Open Font License 1.1 |

Notices required by those licenses:

- **handy-keys**: Copyright (c) 2026 handy-computer. MIT License; the full text is in `vendor/handy-keys/LICENSE`.
- **Lucide**: Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022 as part of Feather (MIT). All other copyright (c) for Lucide are held by Lucide Contributors 2022. Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies. The software is provided "as is" without warranty of any kind.
- **Inter**: Copyright (c) 2016 The Inter Project Authors (https://github.com/rsms/inter). This Font Software is licensed under the SIL Open Font License, Version 1.1, available at https://openfontlicense.org. The font is bundled unmodified and is not sold by itself.
- **Speech models** (downloaded on request, not bundled), used unmodified: NVIDIA Parakeet TDT 0.6B v2 and v3 and NVIDIA Canary 180M Flash, licensed CC-BY-4.0 (https://creativecommons.org/licenses/by/4.0/); OpenAI Whisper Small (MIT); Silero VAD by Snakers4 (MIT). The int8 ONNX conversions are by the sherpa-onnx project on Hugging Face (csukuangfj).
- **MIT and Apache-2.0 components**: each is used under its own license; their copyright notices are kept in their source packages and are available from the package registries.

Design and technique references. No code was copied from these projects unless stated above:

- [Handy](https://github.com/cjpais/Handy) (MIT) informed the shortcut handling, audio capture, clipboard-paste and release-grace ideas, which were reimplemented.
- [Pouchy](https://github.com/editorrylix/Pouchy) informed the tray menu's behaviour (an app-drawn popup that never steals focus). Its code is not used.

Planned components, used under their own licenses when added: llama.cpp (MIT), Silero VAD (MIT), Tesseract (Apache-2.0), SQLite (public domain).
