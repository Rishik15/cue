<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

A small Windows app that takes over repetitive jobs: dictating, fixing text, looking up words, expanding snippets.

[![Windows 10 and 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows&logoColor=white)](https://github.com/Rishik15/cue/releases/latest)
[![Release](https://img.shields.io/github/v/release/Rishik15/cue)](https://github.com/Rishik15/cue/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

Cue lives in your tray and does its work when you press a shortcut. Everything runs on your PC. There is no account, no cloud and no tracking. The network is used only for downloads you ask for.

This is version 0.1, an early release for Windows 10 and 11.

## What it does

| Press | Feature | What you get |
| --- | --- | --- |
| <kbd>Ctrl</kbd> + <kbd>Space</kbd> | Dictation | Hold, talk, let go, and your words appear where the cursor is. An offline speech model does the listening, so nothing is uploaded. Add your own words and Cue drops the "um"s. |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>Space</kbd> | Selection palette | Select text in any app and see what Cue can do with it, with the result previewed next to each choice: fix spacing, change case, join broken PDF lines, strip tracking from links, turn copied web formatting into Markdown. Edit the result, press <kbd>Enter</kbd>, and it is pasted. A small local model can also correct, shorten or reword. |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>G</kbd> | Grab Text | Drag over anything on screen, even text you can't select, and get it back as text. It uses the recognition built into Windows. |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>D</kbd> | Dictionary | Select a word and a small card under it shows its meanings, examples and synonyms. 1.34 million English words, and it works offline. |
| Just type | Snippets | Type `:sig` in any app and it turns into your text, with dates, the clipboard, a cursor position or a question Cue asks first. Triggers can be regular expressions. Bring your snippets from Raycast, Espanso, TextExpander or AutoHotkey. |

The shortcuts use the left <kbd>Ctrl</kbd> and <kbd>Alt</kbd> keys, and you can change all of them on the Shortcuts page.

If a window changes while you dictate, or an app won't accept typing, Cue puts the text on your clipboard so nothing is lost. There are six themes, each with a light and a dark version.

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

Bug reports and ideas are welcome on the [issues page](https://github.com/Rishik15/cue/issues). Cue is not taking pull requests yet.

[Security](SECURITY.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [MIT License](LICENSE)
