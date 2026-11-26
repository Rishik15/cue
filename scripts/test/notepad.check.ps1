# File: scripts/test/notepad.check.ps1
# Purpose: self-check of scripts/lib/notepad.ps1 on real processes: Start-OwnedNotepad starts nothing while a process of that name runs, and Stop-OwnedNotepad ends only the ids it is given.
# Contents: the two cases, the verdict.
# Usage: & scripts/test/notepad.check.ps1   (cue.ps1 test runs it; it never starts or stops a Notepad)
$ErrorActionPreference = "Stop"
$saved = @{ APPDATA = $env:APPDATA; TEMP = $env:TEMP }
$root = Join-Path ([IO.Path]::GetTempPath()) ("cue-notepad-check-" + [guid]::NewGuid())
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$pings = @()
try {
  New-Item -ItemType Directory -Path $root | Out-Null
  $env:APPDATA = $root; $env:TEMP = $root
  . (Join-Path $PSScriptRoot "..\lib\wait.ps1")
  . (Join-Path $PSScriptRoot "..\lib\notepad.ps1")

  $self = (Get-Process -Id $PID).ProcessName
  $before = @(Get-Process $self | ForEach-Object Id)
  $ids = Start-OwnedNotepad $null "never shown" $self
  $new = @(Get-Process $self | Where-Object { $before -notcontains $_.Id })
  Check "a running process of that name: returns nothing" ($null -eq $ids)
  Check "a running process of that name: starts nothing" ($new.Count -eq 0)

  $pings = @(1, 2 | ForEach-Object { Start-Process ping -ArgumentList "-n", "30", "127.0.0.1" -WindowStyle Hidden -PassThru })
  Stop-OwnedNotepad @($pings[0].Id)
  Check "Stop-OwnedNotepad ends the id it is given" ($pings[0].HasExited)
  Check "Stop-OwnedNotepad leaves any other process alive" (-not $pings[1].HasExited)
  Stop-OwnedNotepad @($pings[1].Id)
  Check "Stop-OwnedNotepad ends the second id when asked" ($pings[1].HasExited)
} finally {
  foreach ($p in $pings) { if (-not $p.HasExited) { $p.Kill() } }
  $env:APPDATA = $saved.APPDATA; $env:TEMP = $saved.TEMP
  Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
