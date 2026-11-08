# File: scripts/verify/smoke.ps1
# Purpose: prove the hotkey path end to end on Windows: injected Ctrl+Space must show the native overlay, release must hide it. Needs a release build first (pnpm tauri build --no-bundle). The user's settings are set aside while it runs (the check presses the default shortcut) and put back after.
# Contents: Restore-Settings, Down, Up, Shown, the press and release steps, the verdict.
# Usage: pwsh scripts/verify/smoke.ps1 [-Exe path]
param([string]$exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"))
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $settingsBackup = Join-Path $env:TEMP "cue-settings-backup.json"
$hadSettings = Test-Path $settings
$LCTRL = 0xA2; $SPACE = 0x20; $UP = 2

# Puts the user's settings back, or removes the test settings when there were none.
function Restore-Settings { if (Test-Path $settingsBackup) { Move-Item $settingsBackup $settings -Force } elseif (-not $hadSettings) { Remove-Item $settings -Force -ErrorAction SilentlyContinue } }
# Presses the default dictation shortcut (Ctrl+Space).
function Down { [Win32]::keybd_event($LCTRL, 0x1D, 0, [UIntPtr]::Zero); [Win32]::keybd_event($SPACE, 0x39, 0, [UIntPtr]::Zero) }
# Releases the default dictation shortcut.
function Up { [Win32]::keybd_event($SPACE, 0x39, $UP, [UIntPtr]::Zero); [Win32]::keybd_event($LCTRL, 0x1D, $UP, [UIntPtr]::Zero) }
# Returns whether the native overlay window exists and is visible.
function Shown { [Win32]::IsWindowVisible([Win32]::FindWindow("CueOverlay", [NullString]::Value)) }

if (Test-Path $settings) { Move-Item $settings $settingsBackup -Force }
$p = $null
try {
  New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null
  Set-Content $settings '{"onboarded": true}'
  $mark = Get-LogMark
  $p = Start-Process $exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-smoke.err")
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  Down; $shown = $null -ne (WaitUntil { Shown } 3000)
  Up; $afterRelease = $null -eq (WaitUntil { -not (Shown) } 8000)
  if ($afterRelease) { Down; Up; $afterRelease = $null -eq (WaitUntil { -not (Shown) } 8000) }
} finally {
  if ($p) { $p | Stop-Process -Force -ErrorAction SilentlyContinue }
  Restore-Settings
}
"overlay shown on press: $shown, hidden after stop: $(-not $afterRelease)"
if ($shown -and -not $afterRelease) { "PASS" } else { "FAIL"; exit 1 }
