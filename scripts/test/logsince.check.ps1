# File: scripts/test/logsince.check.ps1
# Purpose: self-check of Read-LogSince (scripts/lib/wait.ps1) on a temp APPDATA: text after a mark with non-ASCII before it, and after a rotation the old log from the mark plus the whole new log (ticket SC-3).
# Contents: the temp log folder, the cases, the verdict.
# Usage: & scripts/test/logsince.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
$saved = $env:APPDATA
$env:APPDATA = Join-Path ([IO.Path]::GetTempPath()) "cue-logsince-check-$PID"
$dir = Join-Path $env:APPDATA "dev.cue.app"
$utf8 = New-Object Text.UTF8Encoding $false
try {
  New-Item -ItemType Directory -Force $dir | Out-Null
  $log = Join-Path $dir "cue.log"
  [IO.File]::WriteAllText($log, "naïve start`n", $utf8)
  $mark = Get-LogMark
  [IO.File]::AppendAllText($log, "press one`n", $utf8)
  Check "no rotation: only the text after the mark" ((Read-LogSince $mark) -ceq "press one`n")
  Move-Item $log (Join-Path $dir "cue.old.log")
  [IO.File]::WriteAllText($log, "after`n", $utf8)
  Check "rotated: old log from the mark plus the new log" ((Read-LogSince $mark) -ceq "press one`nafter`n")
  Remove-Item (Join-Path $dir "cue.old.log")
  Check "rotated with no old log: the new log" ((Read-LogSince $mark) -ceq "after`n")
} finally {
  Remove-Item $env:APPDATA -Recurse -Force -ErrorAction SilentlyContinue
  $env:APPDATA = $saved
}
Complete-Checks
