# File: scripts/verify/smoke.ps1
# Purpose: prove the hotkey path end to end on Windows: in Hold mode, injected Ctrl+Space must show the native overlay and start a real recording (Cue starts its speech worker), and its one release must hide the overlay (no second press, so a release that does nothing fails). Without an installed speech model a press only flashes a notice that hides by itself, so smoke refuses to run then and exits 2 (never PASS). Needs a release build first (pnpm tauri build --no-bundle). The user's settings are set aside while it runs (the check presses the default shortcut) and put back after.
# Contents: Shown, Recording, the installed-model gate, the press and release steps, the verdict.
# Usage: pwsh scripts/verify/smoke.ps1 [-Exe path]
param([string]$exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"))
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\keys.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
. (Join-Path $PSScriptRoot "..\lib\cuelab.ps1")

# Returns whether the native overlay window exists and is visible.
function Shown { [Win32]::IsWindowVisible([Win32]::FindWindow("CueOverlay", [NullString]::Value)) }

# Returns whether the Cue started by this run has a speech worker child, which it starts only for a press that begins a recording (never for the "No speech model" notice).
function Recording { [bool](Get-CimInstance Win32_Process -Filter "ParentProcessId = $($p.Id)" | Where-Object { $_.CommandLine -like "*--speech-worker*" }) }

$models = Join-Path $env:LOCALAPPDATA "dev.cue.app\models"
if (-not (Test-Path -LiteralPath $models -PathType Container) -or -not (Get-CueInstalled $exe $models)) {
  "No speech model is installed, so smoke cannot test recording; install one from Settings > Models and run again."
  "NOT RUN"; exit 2
}

$p = $null
Enter-UserData
try {
  New-Item -ItemType Directory -Force $dir | Out-Null
  Set-Content $settings '{"onboarded": true, "dictation_mode": "Hold"}'
  $mark = Get-LogMark
  $p = Start-Process $exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-smoke.err")
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  Hold @(,$LCTRL) 0x20 0x39
  try { $shown = $null -ne (WaitUntil { Shown } 3000) } finally { Release @(,$LCTRL) 0x20 0x39 }
  $released = $null -ne (WaitUntil { -not (Shown) } 8000)
  $recorded = $null -ne (WaitUntil { Recording } 3000)
} finally {
  if ($p) { $p | Stop-Process -Force -ErrorAction SilentlyContinue }
  Exit-UserData
}
"overlay shown on press: $shown, recording started: $recorded, hidden after release: $released"
if ($shown -and $recorded -and $released) { "PASS" } else { "FAIL"; exit 1 }
