<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

A small Windows app that takes over repetitive jobs: dictating, fixing text, looking up words, expanding snippets.

[![CI](https://github.com/Rishik15/cue/actions/workflows/ci.yml/badge.svg)](https://github.com/Rishik15/cue/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Rishik15/cue)](https://github.com/Rishik15/cue/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

Cue lives in your tray and does its work when you press a shortcut. Everything runs on your PC. There is no account, no cloud and no tracking. The network is used only for downloads you ask for.

This is version 0.1, an early release for Windows 10 and 11.

## What it does

**Dictation.** Hold Left Ctrl + Space, talk, and let go. The words are typed where your cursor is. Speech recognition runs offline with a model you download once (Parakeet, Canary or Whisper). You can add your own words, drop filler like "um", and run saved steps on the result.

**Selection palette.** Select text in any app and press Left Ctrl + Left Alt + Space. Cue lists what it can do with it: fix spacing, change case, join broken PDF lines, pull out links or emails, turn copied web formatting into Markdown. Each choice previews its result, you can edit it, and Enter pastes it. With a small text model installed, it can also correct, shorten or reword.

**Grab Text.** Press Left Ctrl + Left Alt + G and drag over anything on screen. Cue reads the text with Windows' built-in recognition and offers the same actions.

**Snippets.** Type a trigger such as `:sig` in any app and it becomes your text. Triggers can be regular expressions. Snippets can include dates, the clipboard, a cursor position, or a question Cue asks first. You can import from Raycast, Espanso, TextExpander and AutoHotkey.

**Dictionary.** Select a word and press Left Ctrl + Left Alt + D. A small card under the word shows its meanings, examples and synonyms. The English pack (Wiktionary, 1.34 million words) is a 52 MB download on the Dictionary page, and it works offline after that.

If a window changes while you dictate, or an app won't accept typing, Cue puts the text on your clipboard so nothing is lost. All shortcuts can be changed on the Shortcuts page. There are six theme families, each with a light and a dark version.

## Install

Download `Cue_0.1.0_x64-setup.exe` from the [latest release](https://github.com/Rishik15/cue/releases/latest) and run it.

This version is not code-signed, so Windows will say it protected your PC. Choose More info, then Run anyway. Cue downloads nothing unless you press a Download button. Your settings are in `%APPDATA%\dev.cue.app`, and Remove All Cue Data on the About page deletes them.

While it waits in the tray, Cue uses about 4 MB of memory and no CPU.

## Build it

You need Rust (MSVC), Node.js 24 or newer, pnpm and the Visual Studio C++ Build Tools.

```powershell
git clone https://github.com/Rishik15/cue.git
cd cue
./scripts/cue.ps1 setup
./scripts/cue.ps1 dev
./scripts/cue.ps1 build
```

`setup` checks the tools, `dev` runs with hot reload, and `build` makes the exe and installer. The first build takes a few minutes.

## More

[Security](SECURITY.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [MIT License](LICENSE)
