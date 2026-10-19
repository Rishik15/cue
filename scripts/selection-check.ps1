# Purpose: prove the selection shortcut end to end on Windows with no model: a Selection rule (clean links, then uppercase) is saved in rules.json, Notepad
# holds a text with a tracking link, everything is selected, the shortcut (Left Ctrl + Left Alt + Space) is pressed, and the file must hold the rewritten
# text while the clipboard is unchanged; then again with the result sent to the palette, where Ctrl+Enter pastes it; with CUE_CHECK_MODEL=1 and the text model installed, a Correct Text rule must fix a sentence. Your rules.json and settings.json are set aside and restored.
# Needs a release build first (scripts/winsync release). Usage: pwsh scripts/selection-check.ps1 [-Exe path]
param([string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"))
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public class K {
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
}
'@
$dir = Join-Path $env:APPDATA "dev.cue.app"; $rules = Join-Path $dir "rules.json"; $backup = Join-Path $env:TEMP "cue-rules-backup.json"
# The checks press the default shortcuts, so the user's settings are set aside while they run and put back after.
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $settingsBackup = Join-Path $env:TEMP "cue-settings-backup.json"
function Restore-Settings { if (Test-Path $settingsBackup) { Move-Item $settingsBackup $settings -Force } }
if (Test-Path $settings) { Move-Item $settings $settingsBackup -Force }
$checks = [ordered]@{}
function Check($name, $ok) { $script:checks[$name] = [bool]$ok; "{0,-52} {1}" -f $name, $(if ($ok) { "ok" } else { "FAILED" }) }
function Chord($mods, $key, $keyScan) {
  foreach ($m in $mods) { [K]::keybd_event($m[0], $m[1], 0, [UIntPtr]::Zero) }
  [K]::keybd_event($key, $keyScan, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 100; [K]::keybd_event($key, $keyScan, 2, [UIntPtr]::Zero)
  [array]::Reverse($mods); foreach ($m in $mods) { [K]::keybd_event($m[0], $m[1], 2, [UIntPtr]::Zero) }
}
$LCTRL = @(0xA2, 0x1D); $LALT = @(0xA4, 0x38)
# Presses the selection shortcut and reports how long the preview window takes to become visible, counted from the key going down (Cue acts on the press, not the release).
function Press-Timed($mods, $key, $keyScan) {
  foreach ($m in $mods) { [K]::keybd_event($m[0], $m[1], 0, [UIntPtr]::Zero) }
  $sw = [Diagnostics.Stopwatch]::StartNew()
  [K]::keybd_event($key, $keyScan, 0, [UIntPtr]::Zero)
  while ($sw.ElapsedMilliseconds -lt 5000) {
    $h = [K]::FindWindow([NullString]::Value, "Cue Preview")
    if ($h -ne [IntPtr]::Zero -and [K]::IsWindowVisible($h)) { break }
    Start-Sleep -Milliseconds 2
  }
  $ms = $sw.ElapsedMilliseconds
  [K]::keybd_event($key, $keyScan, 2, [UIntPtr]::Zero)
  [array]::Reverse($mods); foreach ($m in $mods) { [K]::keybd_event($m[0], $m[1], 2, [UIntPtr]::Zero) }
  "preview visible {0} ms after the key press" -f $ms
}
function Run-Case($label, $output, $actions = '"clean_links", "upper_case"', $sample = "see https://a.com/p?id=1&utm_source=x now", $pattern = "") {
  @"
{ "version": 1, "rules": [ { "name": "check", "trigger": "selection", "actions": [$actions], "output": "$output" } ] }
"@ | Set-Content $rules
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  if (Test-Path $rules) { "rules file: " + (Get-Content $rules -Raw) }
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-selection.err")
  Start-Sleep 3
  $file = Join-Path $env:TEMP "cue-selection-test.txt"; Set-Content $file $sample -NoNewline
  Start-Process notepad -ArgumentList $file; Start-Sleep 6
  (New-Object -ComObject WScript.Shell).AppActivate("cue-selection-test") | Out-Null; Start-Sleep 1
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  Chord @(,$LCTRL) 0x41 0x1E   # Ctrl+A
  Start-Sleep -Milliseconds 300
  Press-Timed @($LCTRL, $LALT) 0x20 0x39   # the selection shortcut opens the palette on "Selected Text"; the saved action is the row below it
  Start-Sleep -Milliseconds 500
  [K]::keybd_event(0x28, 0x50, 1, [UIntPtr]::Zero); [K]::keybd_event(0x28, 0x50, 3, [UIntPtr]::Zero)   # Down
  Start-Sleep $(if ($pattern) { 20 } else { 3 })   # a text-model row runs after a short pause and needs time
  Chord @(,$LCTRL) 0x0D 0x1C   # Ctrl+Enter pastes the shown result
  Start-Sleep 6   # Notepad types injected text slowly; the text model needs time to load
  Chord @(,$LCTRL) 0x53 0x1F   # Ctrl+S
  Start-Sleep -Milliseconds 1500
  $text = Get-Content $file -Raw
  "$label file: [$text]  stderr: " + ((Get-Content (Join-Path $env:TEMP "cue-selection.err") -ErrorAction SilentlyContinue) -join " | ")
  $want = "SEE HTTPS://A.COM/P?ID=1 NOW"
  Check "$label : file holds what it should" $(if ($pattern) { $text -match $pattern -and $text -ne $sample } else { $text -ieq $want })
  Check "$label : clipboard left as it was" ([System.Windows.Forms.Clipboard]::GetText() -eq "ORIGINAL CLIPBOARD")
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
}
New-Item -ItemType Directory -Force $dir | Out-Null
if (Test-Path $rules) { Copy-Item $rules $backup -Force }
try {
  Run-Case "insert " "insert"
  Run-Case "preview" "preview"
  if ($env:CUE_CHECK_MODEL) {
    Run-Case "model  " "insert" '"correct"' "i has a apple and she dont like it" "^I have an apple"
    # Three sentences, the last one a question: each is edited on its own, and the question is corrected, not answered.
    Run-Case "model 3" "insert" '"correct"' "i has a apple. she dont like it. whats the capital of france" "(?i)^I have an apple\.\s+She doesn't like it\.\s+what'?s the capital of france\??$"
  }
} finally {
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  Restore-Settings
  if (Test-Path $backup) { Copy-Item $backup $rules -Force; Remove-Item $backup } else { Remove-Item $rules -ErrorAction SilentlyContinue }
}
if ($checks.Values -contains $false) { "FAIL"; exit 1 } else { "PASS" }
