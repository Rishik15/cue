<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

A small utility belt for Windows. The little jobs you do by hand, one shortcut away.

[![Windows 10 and 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows&logoColor=white)](https://github.com/Rishik15/cue/releases/latest)
[![Release](https://img.shields.io/github/v/release/Rishik15/cue)](https://github.com/Rishik15/cue/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[Tools](#tools) · [Install](#install) · [Privacy](#privacy) · [Roadmap](#roadmap) · [Build](#build-from-source) · [Changelog](CHANGELOG.md)

</div>

Dictate a sentence. Work out a number. Clean up a paragraph you pasted. Look up a word. Read the text in a screenshot. Type your email signature again.

Each of those usually means another app, another tab or another account. Cue puts them on shortcuts, in one tray app.

- **Local.** Audio, text and screenshots stay on your PC. No account, no cloud, no tracking.
- **Light.** 4.4 MB of memory and 0 % CPU while it waits in the tray.
- **Fast.** 15 ms from shortcut to the recording pill.
- **Free.** Open source, MIT licensed.

## Tools

### Dictation

Hold `Ctrl + Space`, talk, let go. Your words appear at the cursor in any app. An offline speech model does the listening, your own words are remembered and the "um"s are dropped.

### Selection palette

Select text in any app and press `Ctrl + Alt + Space`. Fix spacing, change case, join broken PDF lines, strip tracking from links, turn copied web formatting into Markdown. Each choice shows its result first. An optional local model can correct, shorten or reword.

### Grab Text

Press `Ctrl + Alt + G` and drag over anything on screen. Text you cannot select, in an image, a video or an app that blocks copying, comes back as text you can paste.

### Dictionary

Select a word and press `Ctrl + Alt + D`. A card appears under it with meanings, examples and synonyms. 1.34 million English words, offline.

### Snippets

Type `:sig` in any app and it turns into your text. Snippets can add dates, the clipboard, a cursor position or a question Cue asks first, and triggers can be regular expressions. Bring yours over from Raycast, Espanso, TextExpander or AutoHotkey.

### Notes

Press `Ctrl + Alt + N` for a note that opens at once and saves as you type. Math, lists and timers work on any line:

```text
12 x 8 =                     96
rent: 900
rent / 4 =                   225
10 ft to m                   3.05 m
sum =                        adds the numbers above
[ ] book the flights         Ctrl + Enter ticks it
timer pomodoro x4            25 min focus, 5 rest, four rounds
```

Press `Ctrl + Shift + V` and what you copy in other programs is added to the note until you press it again. Drop a screenshot on a note and the text in it is added. Pasted text is always plain. `Ctrl + P` searches your notes, and deleted ones wait in a trash for 30 days.

Every shortcut can be changed on the Shortcuts page. If a window changes while you dictate, or an app will not take typed text, Cue puts the text on the clipboard so nothing is lost.

## Install

1. Download the installer from the [latest release](https://github.com/Rishik15/cue/releases/latest).
2. Run it. It installs for you or for everyone on the PC.
3. Follow the first-run walkthrough to pick your shortcuts, then press one.

Windows 10 and 11, 64-bit. Models for dictation, the dictionary and the text tools are downloaded only when you press a Download button.

<details>
<summary>Windows says it protected my PC</summary>

Cue is not code-signed yet, so SmartScreen warns about it. Choose **More info**, then **Run anyway**. Each release lists the installer's SHA-256 so you can check the download before you run it.

</details>

<details>
<summary>Updates</summary>

Cue checks for a newer version when it starts and downloads it in the background. It checks the installer's signature, and installs only when you press Update. The check reads one small file from this repository's releases. You can turn it off in About.

</details>

<details>
<summary>Where Cue keeps things</summary>

Settings, snippets and notes are in `%APPDATA%\dev.cue.app`, and downloaded models are in `%LOCALAPPDATA%\dev.cue.app`. Remove All Cue Data on the About page deletes them.

</details>

## Good to know

- Cue is not code-signed yet, so Windows shows a SmartScreen warning on install. Code signing is on the roadmap.
- The dictionary is English only. Dictation models and the text model are optional downloads.
- Cue is in early releases (0.x). It is used every day by its author, and rough edges are expected. Please report them.

## Privacy

- Dictation, text tools, Grab Text, the dictionary, snippets and notes all run on the device. Audio, text and screenshots are not uploaded.
- The network is used for downloads you start (speech models, the dictionary, the optional text model) and one update check at start.
- Grab Text holds the picture in memory for the moment of the read and never writes it to disk.
- Keystrokes are read only while snippet expansion is on, kept in memory for a few seconds at most, and never stored or logged.

The full threat model is in [SECURITY.md](SECURITY.md).

## How light it is

One small program, not a bundled browser. Measured on a release build on Windows 11 (a Core Ultra 7 laptop).

| Measure | Result |
| --- | --- |
| Waiting in the tray | 4.4 MB of memory, 0 % CPU |
| Start to ready | about 0.1 s |
| Shortcut to the recording pill | 15 ms (median) |
| While recording | 6 MB |
| Settings window | opens in 0.3 s, about 150 MB while open, back to one process after closing |
| Installer | 10.7 MB |

The speech and text models run in separate processes that exist only while you use them. A [performance check](.github/workflows/performance.yml) holds every change to these limits.

## Roadmap

Plans, in this order, with no dates.

- [x] Dictation, selection palette, Grab Text, dictionary, snippets, notes
- [x] Updates from inside the app
- [ ] **A shelf for files.** Shake a drag or press a shortcut to collect files while you work, then drag them out into any app.
- [ ] **An optimizer.** Make images, PDFs and videos smaller and convert formats, keeping your originals. It runs from the shelf, on copied images and on watched folders such as Screenshots.
- [ ] **Screen tools.** Circle anything on screen to read or search it, and zoom in and draw on it.
- [ ] Code signing and a winget package.
- [ ] More text actions, read aloud and a Mac version.

Have a little annoyance Cue should fix? Open an [idea](https://github.com/Rishik15/cue/issues/new/choose).

## Build from source

You need Rust (MSVC), Node.js 24 or newer, pnpm and the Visual Studio C++ Build Tools.

```powershell
git clone https://github.com/Rishik15/cue.git
cd cue
./scripts/cue.ps1 setup
./scripts/cue.ps1 dev
./scripts/cue.ps1 build
```

`setup` checks the tools, `dev` runs with hot reload, and `build` makes the exe and the installer. The first build takes a few minutes. Cue is a [Tauri 2](https://tauri.app) app: a Rust backend with a SolidJS and TypeScript front end.

## More

Bug reports and ideas are welcome on the [issues page](https://github.com/Rishik15/cue/issues).

[Changelog](CHANGELOG.md) · [Security](SECURITY.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [MIT License](LICENSE)

Notes was inspired by Antinote.
