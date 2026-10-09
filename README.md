# Cue

**A local desktop utility for dictating, capturing, editing and reusing text.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Cue sits quietly in your system tray and helps you finish small writing and copy/paste tasks inside the apps you already use: speak instead of typing, fix a sentence, grab text from the screen, insert text you use all the time. Everything runs on your device. Text, audio and screenshots are never sent anywhere.

> **Status: early development, version 0.1.0.** The app shell is done and Windows is the only supported platform. Speech recognition and text editing are not built yet, so Cue cannot turn your voice into text today. See [what works now](#what-works-now) and the [roadmap](#roadmap).

## The idea

Most dictation and writing helpers are heavy, always-on and cloud-backed. Cue is the opposite, built on a few principles:

- **Local processing.** Text, audio and captured images stay on the device. The network is used only for explicit model downloads and app updates.
- **Direct interaction.** A shortcut gets you to a useful action with almost no navigation, in the app you are already in.
- **Quiet at rest.** In the tray Cue uses about 2.4 MB of private memory and no measurable CPU. Engines and windows are created when needed and destroyed when done.
- **Preserve intent.** Edits keep your meaning. Exact transformations (case, whitespace, links) are ordinary code, not a language model.
- **One workflow.** Dictation, edits, screen text and saved snippets share the same preview, clipboard and insertion machinery.

## What works now

- **Tray app.** Single instance, optional launch at login. Right-click the tray icon for a short menu (Open Cue, Settings, Quit); left click opens Settings. The menu is drawn natively, with no web view, and costs nothing while closed.
- **Dictation shortcut.** Hold to talk, or press once to start and again to stop (default: Left Ctrl + Space). Choose your own, including Win combinations, left/right-specific modifiers, modifier-only keys and F-keys.
- **Microphone capture.** Pick a device, how long the mic stays open, the maximum length and the shortest recording that counts. Audio is resampled to 16 kHz mono, ready for a speech engine.
- **Recording overlay.** A small native pill with a live level meter, at the bottom or top of the screen. It never takes focus.
- **Text insertion.** Short text is typed into the focused app; longer text is pasted through the clipboard and your previous clipboard (including images and files) is put back. If the target app runs as administrator, Cue says so and leaves the text on the clipboard. Newlines are never typed as Enter, so dictation cannot send a chat message.
- **Settings.** A fast, frameless window with six theme families (each with light and dark), a keyboard-friendly shortcut recorder and a Remove All Cue Data action.

Not yet working: turning speech into text. Debug builds insert a one-line capture summary so the whole path can be tried; release builds record and show the overlay but insert nothing.

## Roadmap

| Milestone | Goal |
| --- | --- |
| **M0 · App shell** (done) | Tray, hotkey, mic capture, overlay, insertion, settings and design system, benchmarks. |
| **M1 · Foundation** | Job and cancellation core, action registry, command bar and result preview, exact text tools (clean whitespace and PDF line breaks, change case, extract emails and URLs, strip tracking from links, convert rich text to Markdown), local storage, and a rule engine that routes text events through actions. |
| **M2 · Voice and editing** | Speech recognition with whisper.cpp, dictation with voice-activity chunking, personal vocabulary, Correct and Rewrite with a local language model (llama.cpp), optional transcript cleanup, spoken edit commands. |
| **M3 · First release** | Grab text from the screen (OCR), saved snippets with import and export, macOS support, full compatibility testing across apps. |
| **M4 · Extensions** | Opt-in clipboard history, folder rules, read aloud, snippet expansion, converters (units, dates, JSON, CSV), summaries and translation. |

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

Run the checks the same way CI does:

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
  audio/             microphone capture and resampling
  shortcut/          global shortcut engine
  platform/          every OS call: insertion, clipboard, overlay, tray popup (Windows)
  tray/              tray icon and menu content
vendor/handy-keys/   keyboard hook library, patched to sleep instead of polling
scripts/             benchmark and smoke-test scripts
```

## Privacy

Cue does not collect telemetry and does not send your text, audio or screenshots anywhere. Logs omit user content. Network access is limited to explicit model downloads and update checks (not yet implemented). Settings live in your user data directory, and Settings → About → Remove All Cue Data deletes them.

## License

Cue is released under the [MIT License](LICENSE).

## Third-party notices

Cue is built on open-source software. The main components and their licenses:

| Component | License |
| --- | --- |
| [Tauri](https://tauri.app/) and its plugins (store, autostart, single-instance) | MIT or Apache-2.0 |
| [SolidJS](https://www.solidjs.com/), [Kobalte](https://kobalte.dev/), [Vite](https://vite.dev/), [Tailwind CSS](https://tailwindcss.com/) | MIT |
| [TypeScript](https://www.typescriptlang.org/) | Apache-2.0 |
| [cpal](https://github.com/RustAudio/cpal) | Apache-2.0 |
| [rubato](https://github.com/HEnquist/rubato), [serde](https://serde.rs/), [windows-rs](https://github.com/microsoft/windows-rs) | MIT or Apache-2.0 |
| [handy-keys](https://github.com/handy-computer/handy-keys) (vendored with one patch, see `vendor/handy-keys/CUE_PATCH.md`) | MIT |
| [Lucide](https://lucide.dev/) icons (interface icons and tray menu icons) | ISC |
| [Inter](https://rsms.me/inter/) variable font (bundled) | SIL Open Font License 1.1 |

Notices required by those licenses:

- **handy-keys**: Copyright (c) 2026 handy-computer. MIT License; the full text is in `vendor/handy-keys/LICENSE`.
- **Lucide**: Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022 as part of Feather (MIT). All other copyright (c) for Lucide are held by Lucide Contributors 2022. Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies. The software is provided "as is" without warranty of any kind.
- **Inter**: Copyright (c) 2016 The Inter Project Authors (https://github.com/rsms/inter). This Font Software is licensed under the SIL Open Font License, Version 1.1, available at https://openfontlicense.org. The font is bundled unmodified and is not sold by itself.
- **MIT and Apache-2.0 components**: each is used under its own license; their copyright notices are kept in their source packages and are available from the package registries.

Design and technique references. No code was copied from these projects unless stated above:

- [Handy](https://github.com/cjpais/Handy) (MIT) informed the shortcut handling, audio capture, clipboard-paste and release-grace ideas, which were reimplemented.
- [Raycast](https://www.raycast.com/) for Windows informed the visual proportions of the settings window and the tray menu.
- [Pouchy](https://github.com/editorrylix/Pouchy) informed the tray menu's behaviour (an app-drawn popup that never steals focus). Its code is not used.

Planned components, used under their own licenses when added: whisper.cpp and llama.cpp (MIT), Silero VAD (MIT), Tesseract (Apache-2.0), SQLite (public domain).
