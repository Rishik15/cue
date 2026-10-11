# Purpose: prove the selection shortcut end to end on Windows with no model: a Selection rule (clean links, then uppercase) is saved in rules.json, Notepad
# holds a text with a tracking link, everything is selected, the shortcut (Left Ctrl + Left Alt + Space) is pressed, and the file must hold the rewritten
# text while the clipboard is unchanged; then again with the result sent to the preview window, where Enter applies it. Your rules.json and settings are backed up and restored.
# Needs a release build first (scripts/winsync release). Usage: pwsh scripts/selection-check.ps1 [-Exe path]
param([string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"))
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public class K { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
'@
$dir = Join-Path $env:APPDATA "dev.cue.app"; $rules = Join-Path $dir "rules.json"; $backup = Join-Path $env:TEMP "cue-rules-backup.json"
$checks = [ordered]@{}
function Check($name, $ok) { $script:checks[$name] = [bool]$ok; "{0,-52} {1}" -f $name, $(if ($ok) { "ok" } else { "FAILED" }) }
function Chord($mods, $key, $keyScan) {
  foreach ($m in $mods) { [K]::keybd_event($m[0], $m[1], 0, [UIntPtr]::Zero) }
  [K]::keybd_event($key, $keyScan, 0, [UIntPtr]::Zero); [K]::keybd_event($key, $keyScan, 2, [UIntPtr]::Zero)
  [array]::Reverse($mods); foreach ($m in $mods) { [K]::keybd_event($m[0], $m[1], 2, [UIntPtr]::Zero) }
}
$LCTRL = @(0xA2, 0x1D); $LALT = @(0xA4, 0x38)
function Run-Case($label, $output) {
  @"
{ "version": 1, "rules": [ { "name": "check", "trigger": "selection", "actions": ["clean_links", "upper_case"], "output": "$output" } ] }
"@ | Set-Content $rules
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru
  Start-Sleep 3
  $file = Join-Path $env:TEMP "cue-selection-test.txt"; Set-Content $file "see https://a.com/p?id=1&utm_source=x now" -NoNewline
  Start-Process notepad -ArgumentList $file; Start-Sleep 6
  (New-Object -ComObject WScript.Shell).AppActivate("cue-selection-test") | Out-Null; Start-Sleep 1
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  Chord @(,$LCTRL) 0x41 0x1E   # Ctrl+A
  Start-Sleep -Milliseconds 300
  Chord @($LCTRL, $LALT) 0x20 0x39   # the selection shortcut
  if ($output -eq "preview") { Start-Sleep 3; Chord @() 0x0D 0x1C }   # the preview window has focus; Enter applies
  Start-Sleep 2
  Chord @(,$LCTRL) 0x53 0x1F   # Ctrl+S
  Start-Sleep -Milliseconds 1500
  $text = Get-Content $file -Raw
  Check "$label : selection replaced by the rule's result" ($text -eq "SEE HTTPS://A.COM/P?ID=1 NOW")
  Check "$label : clipboard left as it was" ([System.Windows.Forms.Clipboard]::GetText() -eq "ORIGINAL CLIPBOARD")
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
}
New-Item -ItemType Directory -Force $dir | Out-Null
if (Test-Path $rules) { Copy-Item $rules $backup -Force }
try {
  Run-Case "insert " "insert"
  Run-Case "preview" "preview"
} finally {
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  if (Test-Path $backup) { Copy-Item $backup $rules -Force; Remove-Item $backup } else { Remove-Item $rules -ErrorAction SilentlyContinue }
}
if ($checks.Values -contains $false) { "FAIL"; exit 1 } else { "PASS" }
