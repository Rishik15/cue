# File: scripts/lib/userdata.ps1
# Purpose: set every file Cue keeps in its data folder (settings, rules, snippets, notes, looked-up words, first-run and update markers) aside in one backup folder before a harness writes its own, and put exactly those back after.
# Contents: $dir, $settings, $noteFiles, $backupDir, Get-UserFiles, Enter-UserData, Exit-UserData, Clear-Notes, Set-HarnessSettings (a harness's settings from settings.json, the one list a Rust test checks against the settings door).
# Usage: . "$PSScriptRoot\..\lib\userdata.ps1" (it loads proc.ps1 for Stop-Cue). Call `Enter-UserData` outside `try` and `Exit-UserData` in its `finally`; an earlier run's leftover backup makes Enter-UserData refuse, so the user's only copy is never overwritten.
. (Join-Path $PSScriptRoot "proc.ps1")
$dir = Join-Path $env:APPDATA "dev.cue.app"
$settings = Join-Path $dir "settings.json"
$noteFiles = "notes.db", "notes.db-wal", "notes.db-shm", "words.db", "words.db-wal", "words.db-shm" | ForEach-Object { Join-Path $dir $_ }
$backupDir = Join-Path $env:TEMP "cue-userdata-backup"

# Lists the files a harness sets aside: every file directly in $dir except Cue's logs (*.log: cue.log, crash.log and their rotated .old.log), which keep a run's diagnostics. Folders (EBWebView) are left alone. Returns the FileInfo items, none when $dir does not exist.
function Get-UserFiles {
  if (-not (Test-Path -LiteralPath $dir)) { return }
  Get-ChildItem -LiteralPath $dir -File -Force -ErrorAction Stop | Where-Object { $_.Extension -ne ".log" }
}

# Stops Cue so no SQLite file is open, then moves every user file (Get-UserFiles) into $backupDir. Throws, touching nothing and leaving a running Cue alone, when $backupDir is left from an earlier run (it may hold the user's only copy); a failed move puts back what was moved, removes the folder and rethrows. Input: process name to stop (the self-check passes one no process has).
function Enter-UserData($name = "cue") {
  if (Test-Path -LiteralPath $backupDir) { throw "A backup from an earlier run is in $backupDir. Move its files back into $dir, delete the folder, then run again." }
  Stop-Cue $name
  New-Item -ItemType Directory -Path $backupDir -ErrorAction Stop | Out-Null
  try { foreach ($f in @(Get-UserFiles)) { Move-Item -LiteralPath $f.FullName -Destination $backupDir -ErrorAction Stop } }
  catch {
    $failed = $_
    Get-ChildItem -LiteralPath $backupDir -Force | Move-Item -Destination $dir -ErrorAction Stop
    Remove-Item -LiteralPath $backupDir -ErrorAction Stop
    throw $failed
  }
}

# Stops Cue, deletes every user file the test run left (Get-UserFiles, so files it created too) and moves the user's back from $backupDir. Throws, restoring nothing, when a test file cannot be deleted (a leftover test WAL beside the user's database would corrupt it). The folder is removed only when every file is back; otherwise it throws naming the files left in it. Input: process name to stop.
function Exit-UserData($name = "cue") {
  Stop-Cue $name
  foreach ($f in @(Get-UserFiles)) { Remove-Item -LiteralPath $f.FullName -Force -ErrorAction SilentlyContinue }
  $stuck = @(Get-UserFiles | ForEach-Object Name)
  if ($stuck) { throw "Could not delete the test run's $($stuck -join ', ') in $dir, so nothing was restored; your files are still in $backupDir. Close what holds them, delete them, move the backup's files back into $dir, then delete the folder." }
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  Get-ChildItem -LiteralPath $backupDir -Force -ErrorAction Stop | Move-Item -Destination $dir -ErrorAction Continue
  $left = @(Get-ChildItem -LiteralPath $backupDir -Force | ForEach-Object Name)
  if ($left) { throw "Could not restore $($left -join ', '); they are still in $backupDir." }
  Remove-Item -LiteralPath $backupDir -ErrorAction Stop
}

# Deletes the test notes and looked-up words so a case starts with none; Cue must not be running.
function Clear-Notes { foreach ($f in $noteFiles) { Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue } }

# Writes settings.json from entries of scripts/lib/settings.json (later entries and then `$extra` win on a shared key), so the keys a harness sets are the ones a Rust test (`harness_settings_pass_the_door`) checks against the settings door. Inputs: the entry name, an optional second entry name, extra keys a harness only knows at run time (the installed model). Throws for an unknown entry.
function Set-HarnessSettings($name, $more = $null, $extra = @{}) {
  $all = Get-Content -LiteralPath (Join-Path $PSScriptRoot "settings.json") -Raw | ConvertFrom-Json
  $values = [ordered]@{}
  foreach ($entry in @($name, $more) | Where-Object { $_ }) {
    if (-not $all.$entry) { throw "scripts/lib/settings.json has no entry $entry" }
    foreach ($prop in $all.$entry.PSObject.Properties) { $values[$prop.Name] = $prop.Value }
  }
  foreach ($k in $extra.Keys) { $values[$k] = $extra[$k] }
  [IO.File]::WriteAllText($settings, (ConvertTo-Json $values -Compress), (New-Object Text.UTF8Encoding $false))
}
