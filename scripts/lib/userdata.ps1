# File: scripts/lib/userdata.ps1
# Purpose: set the user's Cue data (settings, rules, notes and looked-up words) aside in one backup folder before a harness writes its own, and put it back after.
# Contents: $dir, $settings, $userFiles, $noteFiles, $backupDir, Enter-UserData, Exit-UserData, Clear-Notes.
# Usage: . "$PSScriptRoot\..\lib\userdata.ps1" (it loads proc.ps1 for Stop-Cue). Call `Enter-UserData` outside `try` and `Exit-UserData` in its `finally`; an earlier run's leftover backup makes Enter-UserData refuse, so the user's only copy is never overwritten.
. (Join-Path $PSScriptRoot "proc.ps1")
$dir = Join-Path $env:APPDATA "dev.cue.app"
$settings = Join-Path $dir "settings.json"
$userFiles = "settings.json", "rules.json", "notes.db", "notes.db-wal", "notes.db-shm", "words.db", "words.db-wal", "words.db-shm" | ForEach-Object { Join-Path $dir $_ }
$noteFiles = $userFiles | Select-Object -Skip 2
$backupDir = Join-Path $env:TEMP "cue-userdata-backup"

# Stops Cue so no SQLite file is open, then moves every existing user file into $backupDir. Throws, touching nothing, when $backupDir is left from an earlier run (it may hold the user's only copy); a failed move puts back what was moved, removes the folder and rethrows. Input: process name to stop (the self-check passes one no process has).
function Enter-UserData($name = "cue") {
  Stop-Cue $name
  if (Test-Path -LiteralPath $backupDir) { throw "A backup from an earlier run is in $backupDir. Move its files back into $dir, delete the folder, then run again." }
  New-Item -ItemType Directory -Path $backupDir -ErrorAction Stop | Out-Null
  try { foreach ($f in $userFiles) { if (Test-Path -LiteralPath $f) { Move-Item -LiteralPath $f -Destination $backupDir -ErrorAction Stop } } }
  catch {
    $failed = $_
    Get-ChildItem -LiteralPath $backupDir | Move-Item -Destination $dir -ErrorAction Stop
    Remove-Item -LiteralPath $backupDir -ErrorAction Stop
    throw $failed
  }
}

# Stops Cue, deletes the test run's user files and moves the user's back from $backupDir. Throws, restoring nothing, when a test file cannot be deleted (a leftover test WAL beside the user's database would corrupt it). The folder is removed only when every file is back; otherwise it throws naming the files left in it. Input: process name to stop.
function Exit-UserData($name = "cue") {
  Stop-Cue $name
  foreach ($f in $userFiles) { Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue }
  $stuck = @($userFiles | Where-Object { Test-Path -LiteralPath $_ } | Split-Path -Leaf)
  if ($stuck) { throw "Could not delete the test run's $($stuck -join ', ') in $dir, so nothing was restored; your files are still in $backupDir. Close what holds them, delete them, move the backup's files back into $dir, then delete the folder." }
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  Get-ChildItem -LiteralPath $backupDir -ErrorAction Stop | Move-Item -Destination $dir -ErrorAction Continue
  $left = @(Get-ChildItem -LiteralPath $backupDir | ForEach-Object Name)
  if ($left) { throw "Could not restore $($left -join ', '); they are still in $backupDir." }
  Remove-Item -LiteralPath $backupDir -ErrorAction Stop
}

# Deletes the test notes and looked-up words so a case starts with none; Cue must not be running.
function Clear-Notes { foreach ($f in $noteFiles) { Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue } }
