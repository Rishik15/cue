# File: scripts/lib/userdata.ps1
# Purpose: set aside and restore the user's Cue settings, notes and looked-up words, and send key chords, for the harnesses that drive Cue's windows with their own data (selection-check, dictionary-check).
# Contents: $dir, $settings, $settingsBackup, $noteFiles, $LCTRL, $LALT, Restore-Settings, Set-AsideNotes, Clear-Notes, Restore-Notes, Chord.
# Usage: . "$PSScriptRoot\..\lib\userdata.ps1" after win32.ps1 is loaded. selection-check also keeps its rules.json in $dir.
$dir = Join-Path $env:APPDATA "dev.cue.app"
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $settingsBackup = Join-Path $env:TEMP "cue-settings-backup.json"
$LCTRL = @(0xA2, 0x1D); $LALT = @(0xA4, 0x38)

# Puts the user's settings back, or removes the test settings when there were none.
function Restore-Settings { if (Test-Path $settingsBackup) { Move-Item $settingsBackup $settings -Force } else { Remove-Item $settings -Force -ErrorAction SilentlyContinue } }
# The notes database, the dictionary's looked-up words and their write-ahead files.
$noteFiles = "notes.db", "notes.db-wal", "notes.db-shm", "words.db", "words.db-wal", "words.db-shm" | ForEach-Object { Join-Path $dir $_ }
# Moves the user's notes aside once, before the first case; Cue must not be running. A copy left by an earlier run that stopped early is the user's, so it is kept and the leftover test notes are dropped.
function Set-AsideNotes {
  foreach ($f in $noteFiles) {
    $kept = Join-Path $env:TEMP ("cue-check-" + (Split-Path $f -Leaf))
    if ((Test-Path $f) -and -not (Test-Path $kept)) { Move-Item $f $kept -Force } else { Remove-Item $f -Force -ErrorAction SilentlyContinue }
  }
}
# Deletes the test notes so a case starts with none; Cue must not be running.
function Clear-Notes { foreach ($f in $noteFiles) { Remove-Item $f -Force -ErrorAction SilentlyContinue } }
# Puts the user's notes back over the test notes; Cue must not be running.
function Restore-Notes {
  foreach ($f in $noteFiles) {
    Remove-Item $f -Force -ErrorAction SilentlyContinue
    $kept = Join-Path $env:TEMP ("cue-check-" + (Split-Path $f -Leaf))
    if (Test-Path $kept) { Move-Item $kept $f -Force }
  }
}
# Sends a key chord: modifiers down, key down and up, modifiers up. Inputs: modifier (vk, scan) pairs, key code, key scan code.
function Chord($mods, $key, $keyScan) {
  foreach ($m in $mods) { [Win32]::keybd_event($m[0], $m[1], 0, [UIntPtr]::Zero) }
  [Win32]::keybd_event($key, $keyScan, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 100; [Win32]::keybd_event($key, $keyScan, 2, [UIntPtr]::Zero)
  [array]::Reverse($mods); foreach ($m in $mods) { [Win32]::keybd_event($m[0], $m[1], 2, [UIntPtr]::Zero) }
}
