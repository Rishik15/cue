<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

All your utilities in one lightweight Windows app.

[![Windows 10 and 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows&logoColor=white)](https://github.com/Rishik15/cue/releases/latest)
[![Release](https://img.shields.io/github/v/release/Rishik15/cue)](https://github.com/Rishik15/cue/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[Tools](#tools) · [Install](#install) · [Privacy](#privacy) · [Roadmap](#roadmap) · [Build](#build-from-source) · [Changelog](CHANGELOG.md)

<img src="assets/screenshots/settings.png" alt="Cue's settings window" width="760">

</div>

Dictate a sentence. Work out a number. Clean up a paragraph you pasted. Look up a word. Read the text in a screenshot. Type your email signature again.

Each of those usually means another app, another tab or another account. Cue puts them on shortcuts, in one tray app.

- **Local.** Audio, text and screenshots stay on your PC. No account, no cloud, no tracking.
- **Light.** 4.4 MB of memory and 0 % CPU while it waits in the tray.
- **Fast.** 15 ms from shortcut to the recording pill.
- **Free.** Open source, MIT licensed.

## Tools

### Dictation

Hold `Ctrl + Space`, talk, let go. Your words appear at the cursor in any app. The speech model runs offline.

### Selection palette

Select text in any app and press `Ctrl + Alt + Space` to fix spacing, change case, join broken PDF lines, strip tracking from links or turn copied web formatting into Markdown. Each choice shows its result before you paste it.

### Grab Text

Press `Ctrl + Alt + G` and drag over anything on screen. Text you cannot select, in an image or an app that blocks copying, comes back as text you can paste.

### Dictionary

Select a word and press `Ctrl + Alt + D`. A card shows its meanings, examples and synonyms, offline.

### Snippets

Type `:sig` in any app and it turns into your text. Snippets can include dates, the clipboard or a question Cue asks first. Bring yours over from Raycast, Espanso, TextExpander or AutoHotkey.

### Notes

Press `Ctrl + Alt + N` for a note that opens at once and saves as you type. Math, lists and timers work on any line:

```text
12 x 8 =                     96
rent: 900
rent / 4 =                   225
10 ft to m                   3.05 m
[ ] book the flights         Ctrl + Enter ticks it
timer pomodoro x4            25 min focus, 5 rest, four rounds
```

`Ctrl + Shift + V` adds whatever you copy in other programs to the note until you press it again.

Every shortcut can be changed in Settings.

## Install

1. Download the installer from the [latest release](https://github.com/Rishik15/cue/releases/latest).
2. Run it.
3. Follow the first-run walkthrough, then press a shortcut.

Cue is for Windows 10 and 11, 64-bit. Models are downloaded only when you press a Download button.

<details>
<summary>Windows says it protected my PC</summary>

Cue is not code-signed yet, so SmartScreen warns about it. Choose **More info**, then **Run anyway**. Each release lists the installer's SHA-256 if you want to check the download first.

</details>

<details>
<summary>Updates and your data</summary>

Cue checks for a newer version when it starts and installs it only when you press Update. You can turn the check off in About. Settings, snippets and notes live in `%APPDATA%\dev.cue.app`, and Remove All Cue Data in About deletes them.

</details>

## Good to know

- The dictionary is English only. Speech models and the text model are optional downloads.
- Cue is in early releases (0.x). Expect rough edges, and tell me about them on the [issues page](https://github.com/Rishik15/cue/issues).

## Privacy

Everything runs on your PC. The network is used for downloads you start and one update check at start. Nothing you type, say or capture is uploaded. The full details are in [SECURITY.md](SECURITY.md).

## Roadmap

In this order, with no dates.

- [x] Dictation, selection palette, Grab Text, dictionary, snippets, notes
- [ ] A shelf for collecting files as you work and dragging them out
- [ ] An optimizer for shrinking and converting images, PDFs and videos
- [ ] Screen tools: circle to search, zoom and draw
- [ ] Code signing and a winget package

Got a small annoyance Cue should fix? Open an [idea](https://github.com/Rishik15/cue/issues/new/choose).

## Build from source

You need Rust (MSVC), Node.js 24 or newer, pnpm and the Visual Studio C++ Build Tools.

```powershell
git clone https://github.com/Rishik15/cue.git
cd cue
./scripts/cue.ps1 setup
./scripts/cue.ps1 dev
./scripts/cue.ps1 build
```

`setup` checks the tools, `dev` runs with hot reload, and `build` makes the exe and the installer. Cue is a [Tauri 2](https://tauri.app) app with a Rust backend and a SolidJS front end.

[Changelog](CHANGELOG.md) · [Security](SECURITY.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [MIT License](LICENSE)

Notes was inspired by Antinote.
