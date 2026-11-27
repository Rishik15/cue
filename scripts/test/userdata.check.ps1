# File: scripts/test/userdata.check.ps1
# Purpose: self-check of scripts/lib/userdata.ps1 on real files in a throwaway APPDATA and TEMP: a leftover backup is never overwritten and refusing it leaves a running Cue alone (a stand-in process under a unique name), a round trip restores every file Cue writes (named on no list) byte for byte, deletes every file the run created and leaves Cue's logs and folders alone, a bench run seeding its own snippet library gives the user's back byte for byte, a failed move rolls back, a test file that cannot be deleted stops the restore, a run with no user files leaves none behind, and Set-HarnessSettings merges its entries and extra keys and refuses an unknown entry.
# Contents: Reset-Folders, Seed, Same, Start-Running, the seven cases, the verdict.
# Usage: & scripts/test/userdata.check.ps1   (cue.ps1 test runs it; it never touches a real Cue or the user's folders)
$ErrorActionPreference = "Stop"
$saved = @{ APPDATA = $env:APPDATA; TEMP = $env:TEMP }
$root = Join-Path ([IO.Path]::GetTempPath()) ("cue-userdata-check-" + [guid]::NewGuid())
$none = "cue-selfcheck-no-such-process"
$running = $null
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
# Starts a copy of PowerShell under a unique name in $root that sleeps, standing in for a running Cue. Returns the process; outside Windows (no powershell.exe to copy) returns $null.
function Start-Running {
  if ($env:OS -ne "Windows_NT") { return $null }
  $exe = Join-Path $root ("cue-userdata-check-" + [guid]::NewGuid().ToString("N").Substring(0, 8) + ".exe")
  Copy-Item (Join-Path $PSHOME "powershell.exe") $exe
  Start-Process $exe -ArgumentList '-NoProfile -Command "Start-Sleep 120"' -WindowStyle Hidden -PassThru
}

try {
  New-Item -ItemType Directory -Path (Join-Path $root "appdata"), (Join-Path $root "temp") | Out-Null
  $env:APPDATA = Join-Path $root "appdata"; $env:TEMP = Join-Path $root "temp"
  . (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
  $names = "settings.json", "rules.json", "snippets.db", "snippets.db-wal", "notes.db", "notes.db-wal", "notes.db-shm", "words.db", "notes-seen", "last-version", "autostart.seeded"
  $logs = "cue.log", "cue.old.log", "crash.log"

  Reset-Folders
  New-Item -ItemType Directory -Path $backupDir | Out-Null
  $marker = [Text.Encoding]::UTF8.GetBytes("earlier backup"); [IO.File]::WriteAllBytes((Join-Path $backupDir "settings.json"), $marker)
  $seeded = Seed @("settings.json")
  $running = Start-Running
  $refused = $false; try { Enter-UserData $(if ($running) { $running.ProcessName } else { $none }) } catch { $refused = $_.Exception.Message -like "A backup from an earlier run is in*" }
  Check "refuses to run over an earlier backup" $refused
  if ($running) { Check "refusing leaves the running Cue alone" (-not $running.HasExited) } else { "SKIPPED refusing leaves the running Cue alone: no powershell.exe to copy outside Windows" }
  Check "the earlier backup is untouched" (Same (Join-Path $backupDir "settings.json") $marker)
  Check "the current settings stay in place" (Same $settings $seeded["settings.json"])

  Reset-Folders
  $seeded = Seed $names
  $logBytes = Seed $logs
  New-Item -ItemType Directory -Path (Join-Path $dir "EBWebView") | Out-Null
  $page = Join-Path $dir "EBWebView\page"; [IO.File]::WriteAllText($page, "web view data")
  Enter-UserData $none
  Check "round trip: no user file left in the data folder" (-not ($names | Where-Object { Test-Path -LiteralPath (Join-Path $dir $_) }))
  Check "round trip: every user file is in the backup folder" (-not ($names | Where-Object { -not (Same (Join-Path $backupDir $_) $seeded[$_]) }))
  Check "round trip: the logs stay in the data folder" (-not ($logs | Where-Object { -not (Same (Join-Path $dir $_) $logBytes[$_]) }))
  Check "round trip: the backup holds no log or folder" (-not (Get-ChildItem -LiteralPath $backupDir -Force | Where-Object { $_.PSIsContainer -or $_.Extension -eq ".log" }))
  foreach ($n in "settings.json", "notes.db", "snippets.db", "notes-seen", "words.db-wal", "rules.damaged.json") { Set-Content -LiteralPath (Join-Path $dir $n) "test $n" }
  Add-Content -LiteralPath (Join-Path $dir "cue.log") "test run line"
  $grown = [IO.File]::ReadAllBytes((Join-Path $dir "cue.log"))
  Exit-UserData $none
  Check "round trip: every original is back byte for byte" (-not ($names | Where-Object { -not (Same (Join-Path $dir $_) $seeded[$_]) }))
  Check "round trip: files the run created are deleted" (-not ("words.db-wal", "rules.damaged.json" | Where-Object { Test-Path -LiteralPath (Join-Path $dir $_) }))
  Check "round trip: exactly the original files are back" (@(Get-ChildItem -LiteralPath $dir -File -Force).Count -eq $names.Count + $logs.Count)
  Check "round trip: cue.log keeps the run's lines" (Same (Join-Path $dir "cue.log") $grown)
  Check "round trip: folders are left alone" ((Get-Content -LiteralPath $page -Raw) -eq "web view data")
  Check "round trip: the backup folder is gone" (-not (Test-Path -LiteralPath $backupDir))

  Reset-Folders
  $seeded = Seed @("snippets.db", "snippets.db-wal")
  Enter-UserData $none
  Copy-Item (Join-Path $PSScriptRoot "..\..\bench\snippets.db") (Join-Path $dir "snippets.db")
  Set-Content -LiteralPath (Join-Path $dir "snippets.db-wal") "test wal"
  Exit-UserData $none
  Check "bench library: the user's snippet library is back byte for byte" (-not ("snippets.db", "snippets.db-wal" | Where-Object { -not (Same (Join-Path $dir $_) $seeded[$_]) }))
  Check "bench library: no backup folder is left" (-not (Test-Path -LiteralPath $backupDir))

  Reset-Folders
  if ($env:OS -ne "Windows_NT") { "SKIPPED partial failure rolls back: renaming an open file succeeds outside Windows" }
  else {
    $seeded = Seed @("autostart.seeded", "notes.db")
    Check "partial failure: a movable file is enumerated before the locked one" (@(Get-UserFiles)[0].Name -eq "autostart.seeded")
    $handle = [IO.File]::Open((Join-Path $dir "notes.db"), "Open", "ReadWrite", "None")
    $threw = $false
    try { try { Enter-UserData $none } catch { $threw = $true } } finally { $handle.Dispose() }
    Check "partial failure: Enter-UserData throws" $threw
    Check "partial failure: autostart.seeded is moved back" (Same (Join-Path $dir "autostart.seeded") $seeded["autostart.seeded"])
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

  Reset-Folders
  Set-HarnessSettings "bench" "bench-speech" @{ speech_model = "m" }
  $written = Get-Content -LiteralPath $settings -Raw | ConvertFrom-Json
  Check "harness settings: both entries and the extra key are written" ($written.onboarded -eq $true -and $written.mic_linger_secs -eq 1 -and $written.unload_after_secs -eq 5 -and $written.speech_model -eq "m")
  Check "harness settings: an unknown entry throws" $(try { Set-HarnessSettings "nosuch"; $false } catch { $true })
} finally {
  if ($running) { try { $running.Kill() } catch { }; $null = $running.WaitForExit(10000) }
  $env:APPDATA = $saved.APPDATA; $env:TEMP = $saved.TEMP
  Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
