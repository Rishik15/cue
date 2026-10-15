<div align="center">

<img src="assets/icon.png" alt="Cue" width="96" height="96">

# Cue

A local desktop utility for Windows.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

Cue sits in your tray and takes over the small jobs you keep doing by hand. Talk instead of typing, clean up copied text, fix a sentence in any app. Shrinking and collecting files is next. It is built to stay small and fast, and everything runs on your PC. No account, no cloud, no telemetry.

Cue is early (0.1.0, Windows only, no release yet). It is not affiliated with any app it borrows ideas from.

## What it does today

- **Dictation.** Hold a shortcut, speak, let go, and the text is typed where your cursor is. Offline speech models (Parakeet, Canary, Whisper, Nemotron) run in a separate process that only exists while you talk.
- **Selection palette.** Select text in any app and press a shortcut. Cue lists what it can do with that text (clean up, convert, extract, turn copied web or document formatting into Markdown), shows the result next to each choice, and pastes the one you pick. You can edit the result first.
- **Grab Text.** Press a shortcut and drag over any part of the screen. Cue reads the text with the recognition built into Windows (nothing to download), and offers it with the same actions. It copies the one you pick.
- **Snippets.** Type a trigger such as `:sig` in any app and Cue replaces it with your text. Folders, several triggers per snippet, regex triggers, placeholders (dates with offsets, clipboard, cursor, counter, ids, the current app), questions that ask before typing, formatted (Markdown) snippets, optional undo with Backspace. One set of rules for all snippets. Import from Raycast, Espanso, TextExpander and AutoHotkey files, or export as JSON.
- **Saved actions.** When a dictation finishes, or when you press a shortcut on selected text, Cue runs your steps and puts the result back: fix whitespace, join PDF line breaks, change case, remove tracking from links, pull out emails or links.
- **Local text model.** Correct, Make Shorter, Make Formal, Make Friendly and a few more run on a small model on your PC. It starts when needed and stops afterwards.
- **Safe by default.** If you switch windows or the app blocks typing, the text goes to your clipboard instead of the wrong place. Speakers are muted while you record and restored after.
- **Small extras.** Custom vocabulary, filler-word removal, a live caption while you speak, typing stats, six themes.

## Coming next

Files are the next priority, as one feature with two halves:

- **Shelf** (in the manner of Dropover): a floating shelf that collects files while you work. Shake a drag or press a shortcut, keep the files there, drag them out into any app.
- **Optimizer** (in the manner of Clop): make images, PDFs and videos smaller and more compatible with little visible loss, convert between formats (PNG to JPG, Markdown to PDF), fit a file under a size, strip metadata. It runs on the shelf, on copied images and on watched folders such as Screenshots, shows how much it saved, and keeps your originals safe.

After that, screen tools:

- **Circle to search** (in the manner of CircleFlow): circle anything on screen, read its text, decode a QR code, search the words, copy or save the area. Searching with a picture is opt-in because it sends the picture to a third party.
- **Zoom and draw**: zoom into the screen and mark it up.

Later: macOS, more text actions, read aloud. If you want something else, [tell me](https://github.com/Rishik15/cue/issues/new?labels=feedback&title=Feedback%3A%20).

## How light it is

Cue is one small program, not a bundled browser.

| | |
| --- | --- |
| Executable | 34 MB `cue.exe` |
| Installer | about 9 MB when last measured, per user, no admin rights |
| Memory in the tray | 4 MB, 0 % CPU |
| Start to ready | about 100 ms |

Measured on a release build on Windows 11 (Core Ultra 7 laptop).

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
