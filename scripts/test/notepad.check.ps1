# File: scripts/test/notepad.check.ps1
# Purpose: self-check of scripts/lib/notepad.ps1 on real processes: Start-OwnedNotepad starts nothing while a process of that name runs; a launcher with a child window showing the check's own file is owned, a second process of that name started afterwards is neither claimed nor stopped.
# Contents: Encode, Start-Fake, the two cases, the verdict.
# Usage: & scripts/test/notepad.check.ps1   (cue.ps1 test runs it; it never starts or stops a Notepad: its stand-ins are a renamed copy of powershell.exe showing one window each)
$ErrorActionPreference = "Stop"
$saved = @{ APPDATA = $env:APPDATA; TEMP = $env:TEMP }
$root = Join-Path ([IO.Path]::GetTempPath()) ("cue-notepad-check-" + [guid]::NewGuid())
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$fakes = @()

# Returns a script as the base64 text powershell.exe -EncodedCommand takes. Input: the script.
function Encode($script) { [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script)) }

# Starts the stand-in exe running a script, its console hidden. Inputs: exe path, script. Returns the process.
function Start-Fake($exe, $script) { Start-Process $exe -ArgumentList "-NoProfile", "-EncodedCommand", (Encode $script) -WindowStyle Hidden -PassThru }

try {
  New-Item -ItemType Directory -Path $root | Out-Null
  $env:APPDATA = $root; $env:TEMP = $root
  . (Join-Path $PSScriptRoot "..\lib\wait.ps1")
  . (Join-Path $PSScriptRoot "..\lib\win32.ps1")
  . (Join-Path $PSScriptRoot "..\lib\notepad.ps1")

  $self = (Get-Process -Id $PID).ProcessName
  $before = @(Get-Process $self | ForEach-Object Id)
  $ids = Start-OwnedNotepad $null "never shown" $self
  $new = @(Get-Process $self | Where-Object { $before -notcontains $_.Id })
  Check "a running process of that name: returns nothing" ($null -eq $ids)
  Check "a running process of that name: starts nothing" ($new.Count -eq 0)

  $exe = Join-Path $root "cue-notepad-stand-in.exe"
  Copy-Item "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" $exe
  $part = "cue-notepad-check-" + [guid]::NewGuid()
  $window = 'Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; $f.Text = "{0}"; $f.ShowInTaskbar = $false; $f.WindowState = "Minimized"; [Windows.Forms.Application]::Run($f)'
  $launcher = Start-Fake $exe ('$c = Start-Process "{0}" -ArgumentList "-NoProfile", "-EncodedCommand", "{1}" -WindowStyle Hidden -PassThru; $c.WaitForExit()' -f $exe, (Encode ($window -f "$part.txt - Notepad")))
  $fakes += $launcher
  $null = Await "the stand-in to show $part" { Get-OwnedNotepad $launcher.Id $part } 20000
  $owned = Get-OwnedNotepad $launcher.Id $part
  $user = Start-Fake $exe ($window -f "my notes.txt - Notepad")
  $fakes += $user
  $null = Await "the second stand-in to show its window" { [Win32]::Titles([uint32]$user.Id).Count -gt 0 } 20000
  $ownedProcs = @($owned | ForEach-Object { Get-Process -Id $_ })
  $fakes += $ownedProcs
  $child = @(Get-Process cue-notepad-stand-in | Where-Object { [Win32]::Titles([uint32]$_.Id) -like "$part*" } | ForEach-Object Id)
  Check "owned: the launcher and its child window" (($owned[0] -eq $launcher.Id) -and ($child.Count -eq 1) -and ($owned -contains $child[0]))
  Check "owned: a later process of that name is not claimed" (@(Get-OwnedNotepad $launcher.Id $part) -notcontains $user.Id)
  $refused = $false; try { Stop-OwnedNotepad @($user.Id) $part } catch { $refused = $_.Exception.Message -like "Notepad now also shows my notes.txt*" }
  Check "Stop-OwnedNotepad refuses a process showing another file" $refused
  Stop-OwnedNotepad $owned $part
  Check "Stop-OwnedNotepad ends the owned ids" (-not ($ownedProcs | Where-Object { -not $_.HasExited }))
  Check "Stop-OwnedNotepad leaves the later process alive" (-not $user.HasExited)
} finally {
  foreach ($p in $fakes) { if (-not $p.HasExited) { $p.Kill(); $p.WaitForExit() } }
  $env:APPDATA = $saved.APPDATA; $env:TEMP = $saved.TEMP
  Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
