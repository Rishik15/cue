# Changelog

## 0.3.3

This update makes shortcuts and text insertion more reliable, adds an on/off switch for each feature, and makes updates quieter.

### New

- Features: Each feature page has an on/off switch in the top bar. Turning a feature off keeps its shortcut but stops it. Shortcuts can also be turned off one by one.
- Shortcuts: When a combination is already taken, Cue names the shortcut it clashes with

### Improvements

- Text: Inserted text is no longer cut off, and your clipboard is restored more reliably after a paste
- Updates: Installing an update no longer asks for administrator permission, and the download runs at low priority

### Fixes

- Shortcuts: Recording a shortcut is smoother and keyboard input registers more reliably

## 0.3.2

This update fixes keys not registering when recording a shortcut.

### Fixes

- Shortcuts: Fixed keyboard input not registering while recording a shortcut

## 0.3.1

This update makes updating, shortcut recording and inserting text more reliable, especially on older machines.

### Improvements

- Updates: Installing an update no longer opens an installer window, and the notice now only appears on the About page
- Dictation: Speech recognition uses two threads on small PCs, so text appears sooner
- Core: Speech and text engines run at a lower priority so they no longer slow the keyboard or the app you type into

### Fixes

- Shortcuts: Fixed recording a new shortcut failing or never finishing on some machines
- Shortcuts: Cue now says when Windows is not delivering keys to it, and reconnects
- Dictation: Fixed long text being cut off after the first words
- Paste: Fixed text reported as not pasted when a clipboard tool reads it first
- Text actions: Fixed results not returning to the selected text in slow apps

## 0.3.0

This update brings Notes to Cue and makes pasting and typing snippets more reliable on older machines.

### New

- Notes: A floating note on Ctrl + Alt + N with math, lists, timers and AutoPaste

### Improvements

- Snippets: Insert By now offers Paste and Type Slowly, and Type Slowly is the default for new installs
- Snippets: Type Slowly no longer drops or doubles words
- Core: Updated network components to fix certificate security advisories

### Fixes

- Paste: Fixed the old clipboard being pasted instead of your text on older machines
- Dictionary: Fixed the first lookup in a browser opening slightly off the word

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
