<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

All your utilities in one lightweight Windows app.

[![Release](https://img.shields.io/github/v/release/Rishik15/cue)](https://github.com/Rishik15/cue/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

<img src="assets/screenshots/settings.png" alt="Cue's settings window" width="760">

</div>

Cue lives in the tray and does the jobs that usually send you to another app: dictation, text cleanup, notes and math, snippets, a dictionary, and text from anywhere on screen. It runs entirely on your PC with no account and no cloud. Idle, it uses under 5 MB of memory and no CPU.

## Features

- **Dictation.** Hold `Ctrl + Space`, talk, release. The text is typed at the cursor in any app. The speech model runs offline.
- **Selection palette.** Select text, press `Ctrl + Alt + Space`. Fix spacing and case, join broken lines, strip tracking from links, turn copied web content into Markdown. Each option previews its result.
- **Grab Text.** Press `Ctrl + Alt + G` and drag over anything on screen to get its text, including images and apps that block copying.
- **Dictionary.** Select a word, press `Ctrl + Alt + D`. Meanings, examples and synonyms, offline.
- **Snippets.** Type `:sig` anywhere to expand saved text, with the date, clipboard or a prompt. Imports Raycast, Espanso, TextExpander and AutoHotkey.
- **Notes.** Press `Ctrl + Alt + N` for an instant note with math, checklists and timers on any line:

```text
12 x 8 =                     96
rent: 900
rent / 4 =                   225
10 ft to m                   3.05 m
[ ] book the flights         Ctrl + Enter ticks it
timer pomodoro x4            25 min focus, 5 rest, four rounds
```

Every shortcut can be changed in Settings.

## Install

Download the installer from the [latest release](https://github.com/Rishik15/cue/releases/latest) and run it. Requires 64-bit Windows 10 or 11. Speech and text models download only when you press Download.

Cue is not code-signed yet, so SmartScreen warns on first run: choose **More info**, then **Run anyway**. Each release lists the installer's SHA-256.

Cue checks GitHub once at startup for a newer version and installs it only when you press Update. You can turn that off in About. Your data lives in `%APPDATA%\dev.cue.app`, and **Remove All Cue Data** in About deletes it.

## Privacy

Nothing you type, say or capture leaves your PC. The only network use is model downloads you start and the update check. Details in [SECURITY.md](SECURITY.md).

## Build

Requires Rust (MSVC), Node.js 24+, pnpm and the Visual Studio C++ Build Tools.

```powershell
git clone https://github.com/Rishik15/cue.git
cd cue
./scripts/cue.ps1 setup
./scripts/cue.ps1 dev
./scripts/cue.ps1 build
```

Tauri 2, Rust backend, SolidJS frontend. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Status

Early 0.x releases; the dictionary is English only. Planned: a file shelf, an image, PDF and video optimizer, screen tools, code signing and a winget package. Report bugs and ideas on the [issues page](https://github.com/Rishik15/cue/issues).

[Changelog](CHANGELOG.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [MIT License](LICENSE)
