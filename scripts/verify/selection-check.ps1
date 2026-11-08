# File: scripts/verify/selection-check.ps1
# Purpose: prove the selection shortcut end to end on Windows with no model: a Selection rule (clean links, then uppercase) is saved in rules.json, the Cue Note window holds a text with a tracking link, everything is selected, the shortcut (Left Ctrl + Left Alt + Space) is pressed, and the note must hold the rewritten text while the clipboard is unchanged (Cue's own note is the target: a plain text box with no menu bar that could take the keys); then again with the result sent to the palette, where Ctrl+Enter pastes it; with CUE_CHECK_MODEL=1 and the text model installed, a Correct Text rule must fix a sentence. Your rules.json, settings.json and notes are set aside and restored.
# Contents: Restore-Settings, Set-AsideNotes, Clear-Notes, Restore-Notes, Chord, Press-Timed, Note-Focused, Read-Note, Run-Case and the cases.
# Needs a release build first (scripts/cue.ps1 release). Usage: pwsh scripts/verify/selection-check.ps1 [-Exe path]
param([string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"))
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
Add-Type -AssemblyName System.Windows.Forms
$dir = Join-Path $env:APPDATA "dev.cue.app"; $rules = Join-Path $dir "rules.json"; $backup = Join-Path $env:TEMP "cue-rules-backup.json"
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $settingsBackup = Join-Path $env:TEMP "cue-settings-backup.json"
$LCTRL = @(0xA2, 0x1D); $LALT = @(0xA4, 0x38)

# Puts the user's settings back, or removes the test settings when there were none.
function Restore-Settings { if (Test-Path $settingsBackup) { Move-Item $settingsBackup $settings -Force } else { Remove-Item $settings -Force -ErrorAction SilentlyContinue } }
# The notes database and its write-ahead files.
$noteFiles = "notes.db", "notes.db-wal", "notes.db-shm" | ForEach-Object { Join-Path $dir $_ }
# Moves the user's notes aside once, before the first case; Cue must not be running. A copy left by an earlier run that stopped early is the user's, so it is kept and the leftover test notes are dropped.
function Set-AsideNotes {
  foreach ($f in $noteFiles) {
    $kept = Join-Path $env:TEMP ("cue-check-" + (Split-Path $f -Leaf))
    if ((Test-Path $f) -and -not (Test-Path $kept)) { Move-Item $f $kept -Force } else { Remove-Item $f -Force -ErrorAction SilentlyContinue }
  }
}
# Deletes the test notes so a case starts with none; Cue must not be running.
function Clear-Notes { foreach ($f in $noteFiles) { Remove-Item $f -Force -ErrorAction SilentlyContinue } }
# Puts the user's notes back over the test notes; Cue must not be running.
function Restore-Notes {
  foreach ($f in $noteFiles) {
    Remove-Item $f -Force -ErrorAction SilentlyContinue
    $kept = Join-Path $env:TEMP ("cue-check-" + (Split-Path $f -Leaf))
    if (Test-Path $kept) { Move-Item $kept $f -Force }
  }
}
# Sends a key chord: modifiers down, key down and up, modifiers up. Inputs: modifier (vk, scan) pairs, key code, key scan code.
function Chord($mods, $key, $keyScan) {
  foreach ($m in $mods) { [Win32]::keybd_event($m[0], $m[1], 0, [UIntPtr]::Zero) }
  [Win32]::keybd_event($key, $keyScan, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 100; [Win32]::keybd_event($key, $keyScan, 2, [UIntPtr]::Zero)
  [array]::Reverse($mods); foreach ($m in $mods) { [Win32]::keybd_event($m[0], $m[1], 2, [UIntPtr]::Zero) }
}
# Presses the selection shortcut and prints how long the preview window takes to become visible, counted from the key going down (Cue acts on the press, not the release). Throws if it never shows. Inputs: modifier pairs, key code, key scan code.
function Press-Timed($mods, $key, $keyScan) {
  foreach ($m in $mods) { [Win32]::keybd_event($m[0], $m[1], 0, [UIntPtr]::Zero) }
  [Win32]::keybd_event($key, $keyScan, 0, [UIntPtr]::Zero)
  $ms = WaitUntil { $h = [Win32]::FindWindow([NullString]::Value, "Cue Preview"); $h -ne [IntPtr]::Zero -and [Win32]::IsWindowVisible($h) } 5000
  [Win32]::keybd_event($key, $keyScan, 2, [UIntPtr]::Zero)
  [array]::Reverse($mods); foreach ($m in $mods) { [Win32]::keybd_event($m[0], $m[1], 2, [UIntPtr]::Zero) }
  if ($null -eq $ms) { throw "The preview window was not shown 5 s after the selection shortcut." }
  "preview visible {0} ms after the key press" -f $ms
}
# Returns whether the Cue Note window holds the keyboard.
function Note-Focused { $h = [Win32]::FindWindow([NullString]::Value, "Cue Note"); $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h }
# Reads the note's text by copying it (Ctrl+A, Ctrl+C) until it differs from the sample and two reads in a row agree (injected text arrives key by key), then returns it. This changes the clipboard, so check the clipboard first. Inputs: sample text, timeout in ms.
function Read-Note($sample, $ms) {
  $script:prev = $null
  $null = WaitUntil {
    Chord @(,$LCTRL) 0x41 0x1E
    Chord @(,$LCTRL) 0x43 0x2E
    Start-Sleep -Milliseconds 400
    $now = [System.Windows.Forms.Clipboard]::GetText()
    $done = ($now -ne $sample) -and ($now -ceq $script:prev)
    $script:prev = $now
    $done
  } $ms
  [System.Windows.Forms.Clipboard]::GetText()
}
# Runs one rule against the text of a Cue note and checks the note and the clipboard. A new notes file opens with Cue's intro note, so the note is emptied first. The palette result cannot be observed from outside, so a fixed wait before Ctrl+Enter stands in. shortcut: fixed 0.5 s before Down (the page has focus but cannot take keys yet) and fixed 3 s (20 s for a text-model row) before pasting, expose a ready signal from the palette (for example its window title) and wait for it. Inputs: label, rule output, action list, sample text, expected pattern for model rows.
function Run-Case($label, $output, $actions = '"clean_links", "upper_case"', $sample = "see https://a.com/p?id=1&utm_source=x now", $pattern = "") {
  @"
{ "version": 1, "rules": [ { "name": "check", "trigger": "selection", "actions": [$actions], "output": "$output" } ] }
"@ | Set-Content $rules
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  $null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
  Clear-Notes
  "rules file: " + (Get-Content $rules -Raw)
  $mark = Get-LogMark
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-selection.err")
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  Chord @($LCTRL, $LALT) 0x4E 0x31
  $null = Await "the note window to take the keyboard" { Note-Focused } 15000
  Start-Sleep -Milliseconds 800
  Chord @(,$LCTRL) 0x41 0x1E
  [Win32]::keybd_event(0x2E, 0x53, 1, [UIntPtr]::Zero); [Win32]::keybd_event(0x2E, 0x53, 3, [UIntPtr]::Zero)
  [System.Windows.Forms.Clipboard]::SetText($sample)
  Chord @(,$LCTRL) 0x56 0x2F
  Start-Sleep -Milliseconds 500
  Chord @(,$LCTRL) 0x41 0x1E
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  Press-Timed @($LCTRL, $LALT) 0x20 0x39
  $null = Await "the palette to take the keyboard (it first shows unfocusable while the selection is read)" { $h = [Win32]::FindWindow([NullString]::Value, "Cue Preview"); $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 15000
  Start-Sleep -Milliseconds 500
  [Win32]::keybd_event(0x28, 0x50, 1, [UIntPtr]::Zero); [Win32]::keybd_event(0x28, 0x50, 3, [UIntPtr]::Zero)
  Start-Sleep $(if ($pattern) { 20 } else { 3 })
  Chord @(,$LCTRL) 0x0D 0x1C
  Start-Sleep $(if ($pattern) { 20 } else { 3 })
  $clipboardKept = [System.Windows.Forms.Clipboard]::GetText() -eq "ORIGINAL CLIPBOARD"
  $null = Await "the note window to hold the keyboard again" { Note-Focused } 10000
  $text = Read-Note $sample $(if ($pattern) { 60000 } else { 30000 })
  "$label note: [$text]  stderr: " + ((Get-Content (Join-Path $env:TEMP "cue-selection.err") -ErrorAction SilentlyContinue) -join " | ")
  $want = "SEE HTTPS://A.COM/P?ID=1 NOW"
  Check "$label : note holds what it should" $(if ($pattern) { $text -match $pattern -and $text -ne $sample } else { $text -ieq $want })
  Check "$label : clipboard left as it was" $clipboardKept
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
}

if (Test-Path $settings) { Move-Item $settings $settingsBackup -Force }
Set-AsideNotes
New-Item -ItemType Directory -Force $dir | Out-Null
if (Test-Path $rules) { Copy-Item $rules $backup -Force }
try {
  Set-Content $settings '{"onboarded":true}'
  Run-Case "insert " "insert"
  Run-Case "preview" "preview"
  if ($env:CUE_CHECK_MODEL) {
    Run-Case "model  " "insert" '"correct"' "i has a apple and she dont like it" "^I have an apple"
    Run-Case "model 3" "insert" '"correct"' "i has a apple. she dont like it. whats the capital of france" "(?i)^I have an apple\.\s+She doesn't like it\.\s+what'?s the capital of france\??$"
  }
} finally {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  Restore-Settings
  Restore-Notes
  if (Test-Path $backup) { Copy-Item $backup $rules -Force; Remove-Item $backup } else { Remove-Item $rules -ErrorAction SilentlyContinue }
}
Complete-Checks
