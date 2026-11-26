# File: scripts/test/userdata.check.ps1
# Purpose: self-check of scripts/lib/userdata.ps1 on real files in a throwaway APPDATA and TEMP: a leftover backup is never overwritten, a round trip restores every file byte for byte, a failed move rolls back, a test file that cannot be deleted stops the restore, and a run with no user files leaves none behind.
# Contents: Reset-Folders, Seed, Same, the five cases, the verdict.
# Usage: & scripts/test/userdata.check.ps1   (cue.ps1 test runs it; it never touches a real Cue or the user's folders)
$ErrorActionPreference = "Stop"
$saved = @{ APPDATA = $env:APPDATA; TEMP = $env:TEMP }
$root = Join-Path ([IO.Path]::GetTempPath()) ("cue-userdata-check-" + [guid]::NewGuid())
$none = "cue-selfcheck-no-such-process"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")

# Empties the test folders: no user data, no backup folder.
function Reset-Folders {
  Remove-Item -LiteralPath $dir, $backupDir -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
}
# Writes distinct content into each named user file and returns a map of name to bytes. Input: file names under $dir.
function Seed($names) {
  $seeded = @{}
  foreach ($n in $names) { $bytes = [Text.Encoding]::UTF8.GetBytes("original $n " + [guid]::NewGuid()); [IO.File]::WriteAllBytes((Join-Path $dir $n), $bytes); $seeded[$n] = $bytes }
  $seeded
}
# Returns whether a file holds exactly the given bytes. Inputs: path, bytes.
function Same($path, $bytes) { (Test-Path -LiteralPath $path) -and ([Convert]::ToBase64String([IO.File]::ReadAllBytes($path)) -eq [Convert]::ToBase64String($bytes)) }

try {
  New-Item -ItemType Directory -Path (Join-Path $root "appdata"), (Join-Path $root "temp") | Out-Null
  $env:APPDATA = Join-Path $root "appdata"; $env:TEMP = Join-Path $root "temp"
  . (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
  $names = $userFiles | ForEach-Object { Split-Path $_ -Leaf }

  Reset-Folders
  New-Item -ItemType Directory -Path $backupDir | Out-Null
  $marker = [Text.Encoding]::UTF8.GetBytes("earlier backup"); [IO.File]::WriteAllBytes((Join-Path $backupDir "settings.json"), $marker)
  $seeded = Seed @("settings.json")
  $refused = $false; try { Enter-UserData $none } catch { $refused = $_.Exception.Message -like "A backup from an earlier run is in*" }
  Check "refuses to run over an earlier backup" $refused
  Check "the earlier backup is untouched" (Same (Join-Path $backupDir "settings.json") $marker)
  Check "the current settings stay in place" (Same $settings $seeded["settings.json"])

  Reset-Folders
  $seeded = Seed $names
  Enter-UserData $none
  Check "round trip: no user file left in the data folder" (-not ($userFiles | Where-Object { Test-Path -LiteralPath $_ }))
  Check "round trip: every user file is in the backup folder" (-not ($names | Where-Object { -not (Same (Join-Path $backupDir $_) $seeded[$_]) }))
  foreach ($n in "settings.json", "notes.db", "rules.json") { Set-Content -LiteralPath (Join-Path $dir $n) "test $n" }
  Exit-UserData $none
  Check "round trip: every original is back byte for byte" (-not ($names | Where-Object { -not (Same (Join-Path $dir $_) $seeded[$_]) }))
  Check "round trip: the backup folder is gone" (-not (Test-Path -LiteralPath $backupDir))

  Reset-Folders
  if ($env:OS -ne "Windows_NT") { "SKIPPED partial failure rolls back: renaming an open file succeeds outside Windows" }
  else {
    $seeded = Seed @("settings.json", "notes.db")
    $handle = [IO.File]::Open((Join-Path $dir "notes.db"), "Open", "ReadWrite", "None")
    $threw = $false
    try { try { Enter-UserData $none } catch { $threw = $true } } finally { $handle.Dispose() }
    Check "partial failure: Enter-UserData throws" $threw
    Check "partial failure: settings.json is moved back" (Same $settings $seeded["settings.json"])
    Check "partial failure: no backup folder is left" (-not (Test-Path -LiteralPath $backupDir))
    Enter-UserData $none
    Check "partial failure: a second run succeeds once the file is free" (Same (Join-Path $backupDir "notes.db") $seeded["notes.db"])
    Exit-UserData $none
  }

  Reset-Folders
  if ($env:OS -ne "Windows_NT") { "SKIPPED a locked test file stops the restore: deleting an open file succeeds outside Windows" }
  else {
    foreach ($pref in "Stop", "Continue") {
      Reset-Folders
      $seeded = Seed @("notes.db")
      Enter-UserData $none
      foreach ($n in "notes.db", "notes.db-wal") { Set-Content -LiteralPath (Join-Path $dir $n) "test $n" }
      $handle = [IO.File]::Open((Join-Path $dir "notes.db-wal"), "Open", "ReadWrite", "None")
      $threw = $false
      try { try { & { $ErrorActionPreference = $pref; Exit-UserData $none } } catch { $threw = $_.Exception.Message -like "Could not delete the test run's notes.db-wal*" } } finally { $handle.Dispose() }
      Check "locked test WAL ($pref): Exit-UserData throws" $threw
      Check "locked test WAL ($pref): the backup is kept" (Same (Join-Path $backupDir "notes.db") $seeded["notes.db"])
      Check "locked test WAL ($pref): nothing is restored" (-not (Test-Path -LiteralPath (Join-Path $dir "notes.db")))
      Exit-UserData $none
      Check "locked test WAL ($pref): restores once the file is free" ((Same (Join-Path $dir "notes.db") $seeded["notes.db"]) -and -not (Test-Path -LiteralPath (Join-Path $dir "notes.db-wal")))
    }
  }

  Reset-Folders
  Enter-UserData $none
  Set-Content -LiteralPath $settings '{"onboarded":true}'
  Exit-UserData $none
  Check "no user files: the test settings are removed" (-not (Test-Path -LiteralPath $settings))
  Check "no user files: no backup folder is left" (-not (Test-Path -LiteralPath $backupDir))
} finally {
  $env:APPDATA = $saved.APPDATA; $env:TEMP = $saved.TEMP
  Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
