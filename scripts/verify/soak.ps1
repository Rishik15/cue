# File: scripts/verify/soak.ps1
# Purpose: prove the window features do not leak. Opens and closes the Grab Text selector 100 times and compares handles and GDI/USER objects before and after (private memory growth is printed as advisory); no extra Cue process may be left. Your settings.json is replaced by an onboarded one and restored afterwards.
# Contents: state waits that throw on timeout, the warm-up and measured grab cycles, the leak checks.
# Needs a release build first (scripts/cue.ps1 release). Usage: pwsh scripts/verify/soak.ps1 [-Exe path] [-Cycles 100]
param([string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"), [int]$Cycles = 100)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $backup = Join-Path $env:TEMP "cue-soak-settings.json"
$moved = $false
if (Test-Path $settings) { Move-Item $settings $backup -Force; $moved = $true }
$LCTRL = 0xA2; $LALT = 0xA4; $G = 0x47; $ESC = 0x1B

# Sends a key chord: modifiers down, key down and up, modifiers up. Inputs: modifier key codes, key code.
function Chord($mods, $key) {
  foreach ($m in $mods) { [Win32]::keybd_event($m, 0, 0, [UIntPtr]::Zero) }
  [Win32]::keybd_event($key, 0, 0, [UIntPtr]::Zero); [Win32]::keybd_event($key, 0, 2, [UIntPtr]::Zero)
  [array]::Reverse($mods); foreach ($m in $mods) { [Win32]::keybd_event($m, 0, 2, [UIntPtr]::Zero) }
}
# Returns private memory, handle count and GDI/USER object counts of a process. Input: the process.
function Snapshot($p) { $p.Refresh(); [pscustomobject]@{ MB = [math]::Round($p.PrivateMemorySize64 / 1MB, 1); Handles = $p.HandleCount; Gdi = [Win32]::GetGuiResources($p.Handle, 0); User = [Win32]::GetGuiResources($p.Handle, 1) } }
# Returns whether the Grab Text selector window is visible.
function Selector { $h = [Win32]::FindWindow("CueGrabSelector", [NullString]::Value); ($h -ne [IntPtr]::Zero) -and [Win32]::IsWindowVisible($h) }
# Returns whether the Grab Text selector window is the foreground window, which is when it receives Esc.
function Selector-Focused { $h = [Win32]::FindWindow("CueGrabSelector", [NullString]::Value); ($h -ne [IntPtr]::Zero) -and ([Win32]::GetForegroundWindow() -eq $h) }
# Opens the selector with the Grab shortcut, asserts it appeared, cancels it with Esc and asserts it went away. The selector window closing does not mean the grab worker is done (it still frees the screen picture and trims memory, and a press during that is ignored by the app), so a press that shows nothing within 1.5 s is repeated; a repeated press never has an effect of its own. Throws after 6 presses without a selector.
function GrabCycle {
  $shown = $false
  for ($try = 0; $try -lt 6 -and -not $shown; $try++) {
    Chord @($LCTRL, $LALT) $G
    $shown = $null -ne (WaitUntil { Selector } 1500)
  }
  if (-not $shown) { throw "The grab selector did not appear after 6 presses of the Grab shortcut." }
  $null = Await "the grab selector to hold the keyboard" { Selector-Focused } 5000
  Chord @() $ESC
  $null = Await "the grab selector to close" { -not (Selector) } 5000
}

try {
  New-Item -Force -ItemType Directory -Path (Split-Path $settings) | Out-Null
  Set-Content -Path $settings -Value '{"onboarded":true}'
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  $null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
  $mark = Get-LogMark
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru
  Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  1..5 | ForEach-Object { GrabCycle }
  $null = Wait-Stable "private memory to stop moving after warm-up" { (Snapshot $app).MB } 0.5 30000
  $before = Snapshot $app
  1..$Cycles | ForEach-Object { GrabCycle }
  $null = Wait-Stable "private memory to stop moving after the cycles" { (Snapshot $app).MB } 0.5 30000
  $after = Snapshot $app
  "before: $($before | ConvertTo-Json -Compress)"; "after:  $($after | ConvertTo-Json -Compress)"
  "advisory: private memory grew by $([math]::Round($after.MB - $before.MB, 1)) MB over $Cycles cycles (not a pass or fail check; handles, GDI and USER below are)"
  Check "handles grew by 20 or fewer" (($after.Handles - $before.Handles) -le 20)
  Check "GDI objects grew by 5 or fewer" (($after.Gdi - $before.Gdi) -le 5)
  Check "USER objects grew by 5 or fewer" (($after.User - $before.User) -le 5)
  Check "one Cue process only (no worker left behind)" (@(Get-Process cue -ErrorAction SilentlyContinue).Count -eq 1)
} finally {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  if ($moved) { Move-Item $backup $settings -Force } else { Remove-Item $settings -Force -ErrorAction SilentlyContinue }
}
Complete-Checks
