# File: scripts/verify/lifecycle.ps1
# Purpose: the hard check that stays on every pull request: the release app starts, shows its tray, opens and closes settings repeatedly, ends with only its own process, and leaves no orphan process when it is ended while settings is open. It asserts states and process counts, never memory or CPU numbers (those are tracked by bench).
# Contents: settings backup, start, repeated open and close, close and reopen at once, end with settings open, orphan check, verdict.
# Usage: pwsh scripts/verify/lifecycle.ps1 [-Exe path] [-Cycles 3]   (build first: cue.ps1 release)
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"),
  [int]$Cycles = 3
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
if (-not (Test-Path $Exe)) { throw "Build a release exe first (cue.ps1 release). Not found: $Exe" }

Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $backup = Join-Path $env:TEMP "cue-settings-backup.json"
$moved = $false; $p = $null
if (Test-Path $settings) { Move-Item $settings $backup -Force; $moved = $true }
try {
  New-Item -Force -ItemType Directory -Path (Split-Path $settings) | Out-Null
  Set-Content -Path $settings -Value '{"onboarded":true}'
  $p = Start-Process $Exe -ArgumentList "--background" -PassThru
  $null = Await "the launched process to own the tray window" { [Win32]::OwnsWindow([uint32]$p.Id, "tray_icon_app") } 20000
  Check "tray window appears" $true
  $null = Await "the thread count to stop changing after start" { $n = (Snapshot $p.Id $true).Threads; Start-Sleep -Milliseconds 500; $n -eq (Snapshot $p.Id $true).Threads } 30000
  Check "idle app is one process" ((Snapshot $p.Id $true).Procs -eq 1)

  for ($i = 1; $i -le $Cycles; $i++) {
    $null = Open-Settings $p.Id $Exe $true
    Close-Settings
    $null = Await "the web view processes to exit after cycle $i" { (Snapshot $p.Id $true).Procs -eq 1 } 60000
  }
  Check "settings opens and closes $Cycles times, back to one process" ((Snapshot $p.Id $true).Procs -eq 1)

  $null = Open-Settings $p.Id $Exe $true
  for ($i = 1; $i -le $Cycles; $i++) {
    $old = Win $null "Cue"
    Check "close and reopen at once ${i}: close posted" ([Win32]::PostMessage($old, 0x10, [IntPtr]::Zero, [IntPtr]::Zero))
    Start-Process $Exe | Out-Null
    $null = Await "the old settings window to be destroyed in round $i" { -not [Win32]::IsWindow($old) } 60000
    $null = Await "a new settings window to be shown in round $i" { $h = Win $null "Cue"; $h -ne [IntPtr]::Zero -and $h -ne $old -and [Win32]::IsWindowVisible($h) } 120000
    $shown = @([Win32]::WindowsOf([uint32[]]@(Tree $p.Id $true | ForEach-Object Id)) | Where-Object { $_ -like '* "Cue" visible=True' })
    Check "close and reopen at once ${i}: app alive, one window, one app process" ((-not $p.HasExited) -and $shown.Count -eq 1 -and @(Get-Process cue -ErrorAction SilentlyContinue).Count -eq 1)
  }
  $kids = @(Tree $p.Id $true | Where-Object { $_.Id -ne $p.Id } | ForEach-Object Id)
  Check "settings open has web view child processes" ($kids.Count -gt 0)
  Stop-Process -Id $p.Id -Force
  $gone = $null -ne (WaitUntil { -not (Get-Process -Id $kids -ErrorAction SilentlyContinue) } 15000)
  Check "no child process outlives the app" $gone
  Get-Process -Id $kids -ErrorAction SilentlyContinue | Stop-Process -Force
} catch {
  if ($p) { Dump-Failure "lifecycle failed: $($_.Exception.Message)" $p.Id }
  throw
} finally {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  if ($moved) { Move-Item $backup $settings -Force } else { Remove-Item $settings -Force -ErrorAction SilentlyContinue }
}
Complete-Checks
