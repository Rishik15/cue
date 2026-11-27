# File: scripts/verify/smoke.ps1
# Purpose: prove the hotkey path end to end on Windows: in Hold mode, injected Ctrl+Space must show the native overlay and open the microphone while the key is still held (Cue logs "audio: microphone opened" once the stream plays; Cue is fresh, so no earlier stream is open), the overlay must stay up while the key is held (a failed microphone abandons the recording and hides it early), and its one release must hide the overlay (no second press, so a release that does nothing fails). Without an installed speech model a press only flashes a notice that hides by itself, so smoke refuses to run then and exits 2 (never PASS). Needs a release build first (pnpm tauri build --no-bundle). The user's settings are set aside while it runs (the check presses the default shortcut) and put back after.
# Contents: Shown, MicOpened, the installed-model gate, the press and release steps, the verdict.
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

# Returns whether Cue logged a started microphone stream after `$mark` bytes of its log (see Get-LogMark); a log not readable yet counts as no.
function MicOpened($mark) { try { (Read-LogSince $mark) -match $micOpened } catch { $false } }
$micOpened = Get-LogPattern "mic_opened"

$models = Join-Path $env:LOCALAPPDATA "dev.cue.app\models"
if (-not (Test-Path -LiteralPath $models -PathType Container) -or -not (Get-CueInstalled $exe $models)) {
  "No speech model is installed, so smoke cannot test recording; install one from Settings > Models and run again."
  "NOT RUN"; exit 2
}

$p = $null
Enter-UserData
try {
  New-Item -ItemType Directory -Force $dir | Out-Null
  Set-HarnessSettings "smoke"
  $mark = Get-LogMark
  $p = Start-Process $exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-smoke.err")
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  $press = Get-LogMark
  Hold @(,$LCTRL) 0x20 0x39
  try {
    $shown = $null -ne (WaitUntil { Shown } 3000)
    $opened = $null -ne (WaitUntil { MicOpened $press } 5000)
    $held = $shown -and $null -eq (WaitUntil { -not (Shown) } 500)
  } finally { Release @(,$LCTRL) 0x20 0x39 }
  $released = $null -ne (WaitUntil { -not (Shown) } 8000)
} finally {
  if ($p) { $p | Stop-Process -Force -ErrorAction SilentlyContinue }
  Exit-UserData
}
"overlay shown on press: $shown, microphone opened while held: $opened, still shown before release: $held, hidden after release: $released"
if ($shown -and $opened -and $held -and $released) { "PASS" } else { "FAIL"; exit 1 }
