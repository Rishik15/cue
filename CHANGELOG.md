# Changelog

## 0.3.0

This update makes Notes a quick place to work things out, keep lists and collect what you copy, and makes pasting reliable on virtual machines and slow PCs.

### New

- Notes: A floating note on Ctrl + Alt + N with math, named values, units, checklists, timers, links and find.
- Notes: AutoPaste is now a shortcut. Press Ctrl + Shift + V and what you copy in other programs is added to the note until you press it again.
- Notes: The first note is a short tour of everything notes can do.
- Notes: Drop a picture on a note to add the text in it.

### Improvements

- Notes: Pasted text and AutoPaste copies arrive as plain text, without hidden marks or odd spaces.
- Notes: Selecting text highlights only the text, not the side padding, and a click in the margin keeps your caret.
- Notes: Math results no longer show a copy cursor or react to clicks.
- Notes: Timer alerts are on time, with a softer sound.
- Snippets: Insert By now offers Paste and Type Slowly, and Type Slowly is the default for new installs.
- Snippets: Slow typing waits for the erased trigger and presses and releases each key separately, so words are no longer dropped or doubled.

### Fixes

- Paste: Fixed the old clipboard being pasted instead of your text on virtual machines and slow PCs.
- Dictionary: Fixed the first lookup in a browser opening a little away from the word.
- Notes: Fixed a pomodoro with a unit on the rest time, such as `timer 5s 2s x4`.

## 0.2.0

- Cue checks for updates and installs them for you.
- A new layout for the dictionary card.
- Model downloads retry when the connection drops.
- Clearer messages when text cannot be inserted, and a more readable error toast.
- The snippets palette and its shortcut are gone; typed triggers work as before.
- Smaller fixes and a faster settings window.

## 0.1.1

- UI changes.
- Uninstalling can now remove everything Cue stored.

## 0.1.0

The first release, for Windows 10 and 11. It is not code-signed yet.

- Dictation with offline speech models, a live caption, custom words, filler removal and saved steps.
- A selection palette for cleaning up, converting and extracting text, with an optional local text model.
- Grab Text, which reads text from anywhere on screen.
- Snippets with typed triggers, regular expressions, placeholders and imports from Raycast, Espanso, TextExpander and AutoHotkey.
- A dictionary card for any selected word, using an English pack you download once.
- Six theme families, each with light and dark.
