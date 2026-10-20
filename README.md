<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

A small Windows app that takes over repetitive jobs: dictating, fixing text, looking up words, expanding snippets.

[![Windows 10 and 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows&logoColor=white)](https://github.com/Rishik15/cue/releases/latest)
[![Release](https://img.shields.io/github/v/release/Rishik15/cue)](https://github.com/Rishik15/cue/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

Cue lives in your tray and does its work when you press a shortcut. Everything runs on your PC. There is no account, no cloud and no tracking. The network is used only for downloads you ask for and one small check at start for a newer version (you can turn it off in About).

This is version 0.1, an early release for Windows 10 and 11.

## Install

Download `Cue_0.2.0_x64-setup.exe` from the [latest release](https://github.com/Rishik15/cue/releases/latest) and run it. It installs for you, or for everyone on the PC.

This version is not code-signed, so Windows will say it protected your PC. Choose More info, then Run anyway. Cue downloads nothing unless you press a Download button, and the first-run walkthrough sets up the shortcuts. Your settings are in `%APPDATA%\dev.cue.app`, and Remove All Cue Data on the About page deletes them.

## What it does

### Dictation&nbsp;&nbsp;<kbd>Ctrl</kbd>&nbsp;+&nbsp;<kbd>Space</kbd>

Hold, talk, let go, and your words appear where the cursor is. An offline speech model does the listening, so nothing is uploaded. Add your own words and Cue drops the "um"s.

### Selection palette&nbsp;&nbsp;<kbd>Ctrl</kbd>&nbsp;+&nbsp;<kbd>Alt</kbd>&nbsp;+&nbsp;<kbd>Space</kbd>

Select text in any app and see what Cue can do with it, with the result previewed next to each choice: fix spacing, change case, join broken PDF lines, strip tracking from links, turn copied web formatting into Markdown. Edit the result, press <kbd>Enter</kbd>, and it is pasted. A small local model can also correct, shorten or reword.

### Grab Text&nbsp;&nbsp;<kbd>Ctrl</kbd>&nbsp;+&nbsp;<kbd>Alt</kbd>&nbsp;+&nbsp;<kbd>G</kbd>

Drag over anything on screen, even text you can't select, and get it back as text. It uses the recognition built into Windows.

### Dictionary&nbsp;&nbsp;<kbd>Ctrl</kbd>&nbsp;+&nbsp;<kbd>Alt</kbd>&nbsp;+&nbsp;<kbd>D</kbd>

Select a word and a small card under it shows its meanings, examples and synonyms. 1.34 million English words, and it works offline.

### Snippets&nbsp;&nbsp;just type

Type `:sig` in any app and it turns into your text, with dates, the clipboard, a cursor position or a question Cue asks first. Triggers can be regular expressions. Bring your snippets from Raycast, Espanso, TextExpander or AutoHotkey.

### Notes&nbsp;&nbsp;<kbd>Ctrl</kbd>&nbsp;+&nbsp;<kbd>Alt</kbd>&nbsp;+&nbsp;<kbd>N</kbd>

A small floating note that opens at once and saves as you type. Everything works on any line, in any note: end a line with `=` to calculate (`100 + 15% =`, `10 ft to m`, named values like `rent: 900`), `sum =`, `avg =` or `count =` to total the numbers above, `[ ]` for checkboxes, `timer 25m` for a countdown that rings even with the note closed, and a line with just `autopaste` to collect everything you copy. Notes are kept in order, pinned, searched from a palette (Ctrl+P) and moved to a trash first.

The shortcuts use the left <kbd>Ctrl</kbd> and <kbd>Alt</kbd> keys, and you can change all of them on the Shortcuts page.

If a window changes while you dictate, or an app won't accept typing, Cue puts the text on your clipboard so nothing is lost. There are six themes, each with a light and a dark version.

## How light it is

Cue is one small program, not a bundled browser. Measured on a release build on Windows 11 (a Core Ultra 7 laptop).

| Measure | Result |
| --- | --- |
| Waiting in the tray | 4.4 MB of memory, 0 % CPU |
| Start to ready | about 0.1 s |
| Shortcut to the recording pill | 15 ms (median) |
| While recording | 6 MB |
| Settings window | opens in 0.3 s, about 150 MB while open |
| After closing settings | back to one process |
| Installer | 10.7 MB |
| Program | 33 MB |

The speech and text models run in separate processes that exist only while you use them.

## Planned

These come in this order. They are plans, with no dates.

1. **A shelf for files.** Shake a drag or press a shortcut to collect files while you work, then drag them out into any app.
2. **An optimizer.** Make images, PDFs and videos smaller, convert formats such as PNG to JPG, and keep your originals safe. It runs from the shelf, on copied images and on watched folders like Screenshots.
3. **Screen tools.** Circle anything on screen to read or search it, and zoom in and draw on it.
4. **Later.** More text actions, read aloud and a Mac version.

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

Bug reports and ideas are welcome on the [issues page](https://github.com/Rishik15/cue/issues).

[Security](SECURITY.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [MIT License](LICENSE)

## Credits

Inspired by Antinote.
