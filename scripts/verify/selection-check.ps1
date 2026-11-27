# File: scripts/verify/selection-check.ps1
# Purpose: prove the selection shortcut end to end on Windows with no model: a Selection rule (clean links, then uppercase) is saved in rules.json, the Cue Note window holds a text with a tracking link, everything is selected, the shortcut (Left Ctrl + Left Alt + Space) is pressed, and the note must hold the rewritten text while the clipboard is unchanged (Cue's own note is the target: a plain text box with no menu bar that could take the keys); the rule is the strip's first chip: key 1 shows its result and Enter pastes it; with CUE_CHECK_MODEL=1 and the text model installed, a Correct Text rule must fix a sentence. Your rules.json, settings.json and notes are set aside and restored; so is the clipboard's text, and a clipboard holding more than plain text is refused before anything changes.
# Contents: Press-Timed, Run-Case and the cases.
# Needs a release build first (scripts/cue.ps1 release). Usage: pwsh scripts/verify/selection-check.ps1 [-Exe path]
param([string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"))
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\keys.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
Add-Type -AssemblyName System.Windows.Forms
. (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
. (Join-Path $PSScriptRoot "..\lib\clipformats.ps1")
. (Join-Path $PSScriptRoot "..\lib\note.ps1")
$rules = Join-Path $dir "rules.json"
# Presses the selection shortcut and prints how long the strip takes to become visible, counted from the key going down (Cue acts on the press, not the release). Throws if it never shows. Inputs: modifier pairs, key code, key scan code.
function Press-Timed($mods, $key, $keyScan) {
  Hold $mods $key $keyScan
  try { $ms = WaitUntil { Visible $app.Id $null "Cue Transform Text" } 5000 }
  finally { Release $mods $key $keyScan }
  if ($null -eq $ms) { throw "The transform strip was not shown 5 s after the selection shortcut." }
  "strip visible {0} ms after the key press" -f $ms
}
# Runs one rule against the text of a Cue note and checks the note and the clipboard. A new notes file opens with Cue's intro note, so the note is emptied first. Key 1 waits for the strip page's first paint (before it the page has focus but cannot take keys); a model row's Enter waits for Cue to log its result (an exact row's result is on the page the moment key 1 lands, and keys reach the page in order); the note and the clipboard are read once Cue logs the paste. Inputs: label, rule output, action list, sample text, expected pattern for model rows.
function Run-Case($label, $output, $actions = '"clean_links", "upper_case"', $sample = "see https://a.com/p?id=1&utm_source=x now", $pattern = "") {
  @"
{ "version": 1, "rules": [ { "name": "check", "trigger": "selection", "actions": [$actions], "output": "$output" } ] }
"@ | Set-Content $rules
  Stop-Cue
  Clear-Notes
  "rules file: " + (Get-Content $rules -Raw)
  $mark = Get-LogMark
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-selection.err")
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  Fill-Note $mark $sample
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  $pressMark = Get-LogMark
  Press-Timed @($LCTRL, $LALT) 0x20 0x39
  $null = Await "the strip to take the keyboard (it first shows unfocusable while the selection is read)" { $h = Win $app.Id $null "Cue Transform Text"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 15000
  $null = Await "the strip page to paint" { Test-LogLine $pressMark "strip_first_paint" } 10000
  Chord @() 0x31 0x02
  if ($pattern) { $null = Await "the model row's result" { Test-LogLine $pressMark "strip_result" } 60000 }
  Chord @() 0x0D 0x1C
  $null = Await "Cue to paste the result" { Test-LogLine $pressMark "pasted" } 15000
  $clipboardKept = [System.Windows.Forms.Clipboard]::GetText() -eq "ORIGINAL CLIPBOARD"
  $null = Await "the note window to hold the keyboard again" { Note-Focused } 10000
  $text = Copy-All
  "$label note: [$text]  stderr: " + ((Get-Content (Join-Path $env:TEMP "cue-selection.err") -ErrorAction SilentlyContinue) -join " | ")
  $want = "SEE HTTPS://A.COM/P?ID=1 NOW"
  Check "$label : note holds what it should" $(if ($pattern) { $text -match $pattern -and $text -ne $sample } else { $text -ieq $want })
  Check "$label : clipboard left as it was" $clipboardKept
  Stop-Cue
}

Assert-TextClipboard
$clipboard = [System.Windows.Forms.Clipboard]::GetText()
Enter-UserData
try {
  New-Item -ItemType Directory -Force $dir | Out-Null
  Set-Content $settings '{"onboarded":true}'
  Run-Case "insert " "insert"
  if ($env:CUE_CHECK_MODEL) {
    Run-Case "model  " "insert" '"correct"' "i has a apple and she dont like it" "^I have an apple"
    Run-Case "model 3" "insert" '"correct"' "i has a apple. she dont like it. whats the capital of france" "(?i)^I have an apple\.\s+She doesn't like it\.\s+what'?s the capital of france\??$"
  }
} finally {
  try { Exit-UserData } finally { if ($clipboard) { [System.Windows.Forms.Clipboard]::SetText($clipboard) } else { [System.Windows.Forms.Clipboard]::Clear() } }
}
Complete-Checks
