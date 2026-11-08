# File: scripts/verify/selection-check.ps1
# Purpose: prove the selection shortcut end to end on Windows with no model: a Selection rule (clean links, then uppercase) is saved in rules.json, Notepad holds a text with a tracking link, everything is selected, the shortcut (Left Ctrl + Left Alt + Space) is pressed, and the file must hold the rewritten text while the clipboard is unchanged; then again with the result sent to the palette, where Ctrl+Enter pastes it; with CUE_CHECK_MODEL=1 and the text model installed, a Correct Text rule must fix a sentence. Your rules.json and settings.json are set aside and restored.
# Contents: Restore-Settings, Chord, Press-Timed, Save-Settled, Run-Case and the cases.
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
# Saves with Ctrl+S until the file differs from the sample and two reads in a row agree (Notepad types injected text slowly), then returns the text. Inputs: file path, sample text, timeout in ms.
function Save-Settled($file, $sample, $ms) {
  $script:prev = $null
  $null = WaitUntil {
    Chord @(,$LCTRL) 0x53 0x1F
    Start-Sleep -Milliseconds 400
    $now = Get-Content $file -Raw
    $done = ($now -ne $sample) -and ($now -ceq $script:prev)
    $script:prev = $now
    $done
  } $ms
  Get-Content $file -Raw
}
# Runs one rule against Notepad text and checks the file and the clipboard. The palette result cannot be observed from outside, so a fixed wait before Ctrl+Enter stands in. shortcut: fixed 0.5 s before Down (the page has focus but cannot take keys yet) and fixed 3 s (20 s for a text-model row) before pasting, expose a ready signal from the palette (for example its window title) and wait for it. Inputs: label, rule output, action list, sample text, expected pattern for model rows.
function Run-Case($label, $output, $actions = '"clean_links", "upper_case"', $sample = "see https://a.com/p?id=1&utm_source=x now", $pattern = "") {
  @"
{ "version": 1, "rules": [ { "name": "check", "trigger": "selection", "actions": [$actions], "output": "$output" } ] }
"@ | Set-Content $rules
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  $null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
  "rules file: " + (Get-Content $rules -Raw)
  $mark = Get-LogMark
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-selection.err")
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  $file = Join-Path $env:TEMP "cue-selection-test.txt"; Set-Content $file $sample -NoNewline
  Start-Process notepad -ArgumentList $file
  $null = Await "Notepad to show the test file" { @(Get-Process notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like "*cue-selection-test*" }).Count -gt 0 } 20000
  $null = Await "Notepad to take the focus" { (New-Object -ComObject WScript.Shell).AppActivate("cue-selection-test") } 5000
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  Chord @(,$LCTRL) 0x41 0x1E
  Press-Timed @($LCTRL, $LALT) 0x20 0x39
  $null = Await "the palette to take the keyboard (it first shows unfocusable while the selection is read)" { $h = [Win32]::FindWindow([NullString]::Value, "Cue Preview"); $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 15000
  Start-Sleep -Milliseconds 500
  [Win32]::keybd_event(0x28, 0x50, 1, [UIntPtr]::Zero); [Win32]::keybd_event(0x28, 0x50, 3, [UIntPtr]::Zero)
  Start-Sleep $(if ($pattern) { 20 } else { 3 })
  Chord @(,$LCTRL) 0x0D 0x1C
  $text = Save-Settled $file $sample $(if ($pattern) { 60000 } else { 30000 })
  "$label file: [$text]  stderr: " + ((Get-Content (Join-Path $env:TEMP "cue-selection.err") -ErrorAction SilentlyContinue) -join " | ")
  $want = "SEE HTTPS://A.COM/P?ID=1 NOW"
  Check "$label : file holds what it should" $(if ($pattern) { $text -match $pattern -and $text -ne $sample } else { $text -ieq $want })
  Check "$label : clipboard left as it was" ([System.Windows.Forms.Clipboard]::GetText() -eq "ORIGINAL CLIPBOARD")
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
}

if (Test-Path $settings) { Move-Item $settings $settingsBackup -Force }
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
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  Restore-Settings
  if (Test-Path $backup) { Copy-Item $backup $rules -Force; Remove-Item $backup } else { Remove-Item $rules -ErrorAction SilentlyContinue }
}
Complete-Checks
