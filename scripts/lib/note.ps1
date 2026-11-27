# File: scripts/lib/note.ps1
# Purpose: drive a Cue Note from a harness: whether it holds the keyboard, copy its text, and fill it with a text once its editor takes keys, each waiting on a state (Cue's log, the clipboard) instead of a fixed time.
# Contents: Note-Focused, Copy-All, Fill-Note.
# Usage: dot-source after wait.ps1, win32.ps1, keys.ps1 and proc.ps1, with System.Windows.Forms loaded and the started Cue in $app.
# Returns whether the Cue Note window holds the keyboard.
function Note-Focused { $h = Win $app.Id $null "Cue Note"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h }
# Selects all of the focused field and copies it (Ctrl+A, Ctrl+C), waiting until the copy lands, so every key sent before it (Cue's injected text included) has landed too. Returns the copied text. Changes the clipboard.
function Copy-All {
  Chord @(,$LCTRL) 0x41 0x1E
  $seq = [Win32]::GetClipboardSequenceNumber(); Chord @(,$LCTRL) 0x43 0x2E; Wait-Copied $seq
  [System.Windows.Forms.Clipboard]::GetText()
}
# Opens a Cue Note (Ctrl+Alt+N), waits until its editor holds the keyboard (Cue logs it once the page has focused the editor), replaces its text with `$text` by pasting and waits until the paste has landed; the text ends selected. Changes the clipboard. Inputs: the log mark from before the press, the text.
function Fill-Note($mark, $text) {
  Chord @($LCTRL, $LALT) 0x4E 0x31
  $null = Await "the note window to take the keyboard" { Note-Focused } 15000
  $null = Await "the note editor to take the keyboard" { Test-LogLine $mark "note_focused" } 15000
  Chord @(,$LCTRL) 0x41 0x1E
  Tap 0x2E 0x53 $true
  [System.Windows.Forms.Clipboard]::SetText($text)
  Chord @(,$LCTRL) 0x56 0x2F
  $null = Copy-All
}
