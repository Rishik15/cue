# Security

Cue runs on your device. It has no account and no telemetry, and sends no text, audio or screenshots anywhere. The network is used only for model downloads you start and, unless you turn it off, one request at startup asking GitHub whether a newer version exists. Updates are checked against a signature and run only when you press Update.

## Supported versions

Only the latest release gets security fixes.

## Reporting a vulnerability

Report it privately through [GitHub's security advisory form](https://github.com/Rishik15/cue/security/advisories/new), not a public issue, with steps to reproduce. Expect a first reply within a few days.

## What Cue does

- **Logs.** `cue.log` and `crash.log` are capped at 256 KB with one old copy and hold no dictated, selected, typed or captured text.
- **Your data.** Settings, snippets and notes are local files in `%APPDATA%\dev.cue.app`; Remove All Cue Data in About deletes them. Grab Text keeps the screen capture in memory only until the text is read by Windows' built-in recognition.
- **Text model.** The text model has no tools and no access to files, the network or other programs, so injected instructions can at worst change the wording of one edit. Text goes in one sentence at a time and never leaves your PC. Each answer is checked before it replaces anything and dropped if it refuses, answers instead of edits, changes too much, or loses a link, number or code span. Hidden and control characters are stripped both ways.
- **Local model server.** It runs as a separate llama.cpp process, listens on `127.0.0.1` only, requires a random key per start, and is tied to Cue by a Windows job object.
- **Typing into other apps.** Every insertion and clipboard write passes one filter that removes control characters. Newlines are never typed as Enter. Submit After Inserting is off by default. Cue will not type into apps running as administrator; it uses the clipboard and tells you. Cue puts your previous clipboard back after a paste only when the clipboard still holds Cue's temporary text, so it never overwrites something you copied since. If a clipboard history tool reads the pasted text first, Cue leaves it in place, because nothing says when the app has finished reading; that text then stays on the clipboard.
- **Snippet expansion.** It reads keystrokes through Windows Raw Input only while the setting is on and a snippet has a trigger. At most 64 recent characters are held in memory, cleared at every break, and never written to disk or logged. It is skipped in password fields, listed programs and admin apps.
- **Windows and commands.** Each window can call only the commands it needs: the build fails when a capability grant differs from the declared list. A content security policy forbids remote scripts. Settings are validated against an allow-list of keys, types and ranges, and a settings file written by a newer Cue is never overwritten.
- **Downloads.** HTTPS only, pinned addresses, and every file is checked against a pinned SHA-256 before use.
- **Dependencies and storage.** Lockfiles are committed; CI runs `cargo deny` and `cargo machete`. SQL uses bound parameters only. DLLs load only from the app folder and System32.
- **Limits.** Input sizes are capped: a selection copy borrows at most 16 MB of clipboard, 200,000 characters per selection, 20,000 to the model, 1 MB for `rules.json`, 5 MB for imported snippet files.

## What Cue does not defend against

- Malware running as you. It can read your clipboard and Cue's files. Cue is not a sandbox against your own account.
- A model that is simply wrong. The checks stop a lost link or number, not a changed word. Use the preview for text that matters.
- Apps that log keystrokes. Cue types like a keyboard, so they see what it types.
