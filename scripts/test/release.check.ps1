# File: scripts/test/release.check.ps1
# Purpose: self-check of scripts/lib/release.ps1 in a temp folder: a fresh exe and an exe older than a source file or a manifest.
# Contents: the temp repo, the cases, the verdict.
# Usage: & scripts/test/release.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\release.ps1")
$root = Join-Path ([IO.Path]::GetTempPath()) "cue-release-check-$PID"
try {
  $exe = Join-Path $root "src-tauri\target\release\cue.exe"
  $source = Join-Path $root "src-tauri\src\main.rs"
  $manifest = Join-Path $root "package.json"
  foreach ($f in $exe, $source, $manifest) { New-Item -ItemType File -Force $f | Out-Null }
  $old = (Get-Date).AddHours(-1)
  foreach ($f in $source, $manifest) { (Get-Item $f).LastWriteTime = $old }
  Check "a fresh exe is not stale" ((Get-StaleSource $root $exe) -eq "")
  (Get-Item $source).LastWriteTime = (Get-Date).AddMinutes(5)
  Check "a newer source file makes it stale" ((Get-StaleSource $root $exe) -eq (Get-Item $source).FullName)
  (Get-Item $source).LastWriteTime = $old
  (Get-Item $manifest).LastWriteTime = (Get-Date).AddMinutes(5)
  Check "a newer manifest makes it stale" ((Get-StaleSource $root $exe) -eq (Get-Item $manifest).FullName)
} finally {
  Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
