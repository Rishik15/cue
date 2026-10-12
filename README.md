<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

A local desktop utility for Windows.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

Cue sits in your tray and takes over the small jobs you keep doing by hand. Talk instead of typing, clean up copied text, fix a sentence in any app. File rules that sort your Downloads folder are next. It is my take on what Hazel does on macOS, built to stay small and fast, and everything runs on your PC. No account, no cloud, no telemetry.

Cue is early (0.1.0, Windows only, no release yet). It is not affiliated with any app it borrows ideas from.

## What it does today

- **Dictation.** Hold a shortcut, speak, let go, and the text is typed where your cursor is. Offline speech models (Parakeet, Canary, Whisper, Nemotron) run in a separate process that only exists while you talk.
- **Rules.** When a dictation finishes, or when you press a shortcut on selected text, Cue runs your steps and puts the result back. Steps include fixing whitespace, joining PDF line breaks, changing case, removing tracking from links and pulling out emails or links.
- **Local text model.** Correct, Make Shorter, Make Formal and Make Friendly run on a small model on your PC. It starts when a rule needs it and stops afterwards.
- **Safe by default.** If you switch windows or the app blocks typing, the text goes to your clipboard instead of the wrong place. Speakers are muted while you record and restored after.
- **Small extras.** Custom vocabulary, filler-word removal, a live caption while you speak, a preview before applying, typing stats, six themes.

## Coming next

- File rules: watch Downloads and other folders, route files by type, name or source, keep a folder organized when you drop things in, with a dry run and undo.
- Command bar, saved snippets, grab text from the screen (OCR), rich text to Markdown.
- macOS.

After that: clipboard history, read aloud, snippet expansion, converters, summaries and translation. If you want something else, [tell me](https://github.com/Rishik15/cue/issues/new?labels=feedback&title=Feedback%3A%20).

## How light it is

Measured on a release build on Windows 11 (Core Ultra 7 laptop).

| | |
| --- | --- |
| Memory in the tray | 4 MB private (15 MB working set), 0 % CPU |
| Start to ready | 138 ms |
| Recording pill appears | 4 ms after the shortcut |
| Program size | 26 MB `cue.exe`, INSTALLER installer (per user, no admin rights) |
| Settings window open | about 157 MB, all freed when you close it |
| While dictating | the model's memory only: about 0.7 GB for Parakeet v2, gone when it unloads |
| Model downloads (optional) | 210 MB to 670 MB for speech, 1.1 GB for a text model |
| Accuracy of the default speech model | Parakeet v2, 2.9 % word error rate on 73 clean English clips, about 19x real time |

Models are not bundled. You pick and download them in the app, and they live in your user data folder, not in the install folder.

## Build it

You need Rust (MSVC), Node.js 24+, pnpm and the Visual Studio C++ Build Tools. Then:

```powershell
git clone https://github.com/Rishik15/cue.git
cd cue
./scripts/cue.ps1 setup     # checks the tools, installs packages
./scripts/cue.ps1 dev       # run with hot reload
./scripts/cue.ps1 build     # exe and installer in src-tauri\target\release
```

`check` and `test` run what CI runs. The first build takes a few minutes and a release build about five, because it links with full optimization.

## More

[Security](SECURITY.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [MIT License](LICENSE)
