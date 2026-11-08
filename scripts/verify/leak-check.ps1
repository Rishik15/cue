# File: scripts/verify/leak-check.ps1
# Purpose: prove the dictation shortcut never types its own key into the focused app. Ctrl+Space is held, Ctrl is let go first (the usual way to end a hold), Space keeps auto-repeating for a moment, then is released; Notepad must stay empty. Your settings.json is replaced by an onboarded one and restored afterwards. A final "x" typed after the release proves the earlier keys were all processed, so an empty file cannot mean "nothing arrived yet".
# Contents: Key, Restore-Settings, the Notepad wait, the key sequence, the save-and-read wait, the verdict.
# Needs a release build first (scripts/cue.ps1 release). Usage: pwsh scripts/verify/leak-check.ps1 [-Exe path]
param([string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"))
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")

# Sends one key event. Inputs: virtual key code, scan code, whether it is a key-up.
function Key($vk, $scan, $up) { [Win32]::keybd_event($vk, $scan, $(if ($up) { 2 } else { 0 }), [UIntPtr]::Zero) }

# Puts the user's settings back, or removes the test settings when there were none.
function Restore-Settings { if (Test-Path $settingsBackup) { Move-Item $settingsBackup $settings -Force } elseif (-not $hadSettings) { Remove-Item $settings -Force -ErrorAction SilentlyContinue } }

$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $settingsBackup = Join-Path $env:TEMP "cue-leak-settings-backup.json"
$hadSettings = Test-Path $settings
Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
$file = Join-Path $env:TEMP "cue-leak-test.txt"; Set-Content $file "" -NoNewline
if ($hadSettings) { Move-Item $settings $settingsBackup -Force }
try {
  New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null
  Set-Content $settings '{"onboarded": true}'
  $mark = Get-LogMark
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  Start-Process notepad -ArgumentList $file
  $null = Await "Notepad to show the test file" { @(Get-Process notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like "*cue-leak-test*" }).Count -gt 0 } 20000
  $null = Await "Notepad to take the focus" { (New-Object -ComObject WScript.Shell).AppActivate("cue-leak-test") } 5000
  Key 0xA2 0x1D $false; Key 0x20 0x39 $false
  Start-Sleep -Milliseconds 400
  Key 0xA2 0x1D $true
  1..8 | ForEach-Object { Start-Sleep -Milliseconds 40; Key 0x20 0x39 $false }
  Key 0x20 0x39 $true
  Key 0x58 0x2D $false; Key 0x58 0x2D $true
  Key 0xA2 0x1D $false; Key 0x53 0x1F $false; Key 0x53 0x1F $true; Key 0xA2 0x1D $true
  $null = WaitUntil { (Get-Content $file -Raw) -match "x" } 10000
  $text = Get-Content $file -Raw
  "file holds: [$text]"
  if ($text -ceq "x") { "PASS" } else { "FAIL"; exit 1 }
} finally {
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  Restore-Settings
}
