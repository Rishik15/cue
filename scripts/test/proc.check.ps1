# File: scripts/test/proc.check.ps1
# Purpose: self-check of scripts/lib/proc.ps1: Percentile, which every bench number goes through (the nearest-rank median and 95th percentile of 1..10, and $null for an empty list), and the peak sampler failing loudly when a sample fails, Open-Settings counting a memory peak that comes and goes before the window shows (a stand-in process the check owns, with a child for the web view, commits 300 MB, frees it, then shows a window titled "Cue" when Open-Settings launches its trigger) and timing the stand-in's window, not an unrelated window titled "Cue" that is already open, Close-Settings closing only the stand-in's window while the unrelated one stays open, Win refusing two matching windows and matching the class, and Stop-Cue waiting out a killed process that stays listed after it has exited (a stand-in with 1.5 GB touched takes 100-200 ms to leave the list).
# Contents: the Percentile cases, the failed-sample cases, Start-Owned (a minimized PowerShell the caller owns), Open-StandIn (the stand-in round trip through Open-Settings), the ownership and peak cases, Stop-BigStandIn and the Stop-Cue case, the verdict.
# Usage: & scripts/test/proc.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
Check "the 50th percentile of 1..10 is 5" ((Percentile (10..1) 50) -eq 5)
Check "the 95th percentile of 1..10 is 10" ((Percentile (1..10) 95) -eq 10)
Check "the percentile of an empty list is null" ($null -eq (Percentile @() 50))

# A root id Get-Process cannot bind makes Snapshot fail inside the real sampler.
$sampler = Start-PeakSampler "no such id" $false
Check "a failed sample stops the sampler" ($null -ne (WaitUntil { $sampler.Handle.IsCompleted } 10000))
Check "a failed sample is thrown by Stop-PeakSampler" $(try { $null = Stop-PeakSampler $sampler; $false } catch { $true })

# Starts a script in a minimized PowerShell and adds the process to the caller's list before anything else runs, so the caller's finally ends it however the check ends. Inputs: the script path, the caller's list. Returns the process.
function Start-Owned($script, $owned) {
  $p = Start-Process (Join-Path $PSHOME "powershell.exe") -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$script`"" -WindowStyle Minimized -PassThru
  $owned.Add($p)
  $p
}

# Opens an unrelated window titled "Cue" first, then starts a stand-in (with a hidden child that lives as long as it does, standing in for the web view) that commits 300 MB, frees it and shows a window titled "Cue" once Open-Settings launches its trigger, measures it with the stand-in as the root, and closes the settings with Close-Settings on that root. Every process it starts is ended in finally with .NET calls (a cmdlet such as Stop-Process fails in a finally once Ctrl+C has stopped the pipeline), also after a throw or Ctrl+C. Returns what Open-Settings returned (Open), whether the stand-in owned the "Cue" window (Mine), whether Win refused the two "Cue" windows of both owners (Both) and found none with a wrong class (ClassMiss), whether it exited (Exited) and whether the unrelated window stayed open (OtherStays).
function Open-StandIn {
  $folder = Join-Path $env:TEMP ("cue-proc-check-" + [guid]::NewGuid())
  $owned = [Collections.Generic.List[Diagnostics.Process]]::new()
  New-Item -ItemType Directory -Path $folder | Out-Null
  try {
    Set-Content (Join-Path $folder "window.ps1") 'Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; $f.Text = "Cue"; [Windows.Forms.Application]::Run($f)'
    Set-Content (Join-Path $folder "standin.ps1") 'Start-Process (Join-Path $PSHOME "powershell.exe") -ArgumentList "-NoProfile -Command Wait-Process -Id $PID" -WindowStyle Hidden; while (-not (Test-Path "$PSScriptRoot\go")) { Start-Sleep -Milliseconds 5 }; $m = [Runtime.InteropServices.Marshal]::AllocHGlobal(300MB); Start-Sleep -Milliseconds 1500; [Runtime.InteropServices.Marshal]::FreeHGlobal($m); & "$PSScriptRoot\window.ps1"'
    Set-Content (Join-Path $folder "go.cmd") '@type nul > "%~dp0go"'
    $other = Start-Owned (Join-Path $folder "window.ps1") $owned
    $null = Await "the unrelated window titled Cue" { [Win32]::Titles($other.Id) -contains "Cue" } 20000
    $standIn = Start-Owned (Join-Path $folder "standin.ps1") $owned
    $open = Open-Settings $standIn.Id (Join-Path $folder "go.cmd")
    $mine = [Win32]::Titles($standIn.Id) -contains "Cue"
    $both = try { $null = Win @($other.Id, $standIn.Id) $null "Cue"; $false } catch { $true }
    $classMiss = (Win $other.Id "NoSuchClass" "Cue") -eq [IntPtr]::Zero
    Close-Settings $standIn.Id
    $exited = $standIn.WaitForExit(10000)
    [pscustomobject]@{ Open = $open; Mine = $mine; Both = $both; ClassMiss = $classMiss; Exited = $exited; OtherStays = -not $other.HasExited -and ([Win32]::Titles($other.Id) -contains "Cue") }
  } finally {
    foreach ($p in $owned) { try { $p.Kill() } catch { }; $null = $p.WaitForExit(10000) }
    [IO.Directory]::Delete($folder, $true)
  }
}

$standIn = Open-StandIn
$open = $standIn.Open
Check "the stand-in owns the window titled Cue" $standIn.Mine
Check "Win throws when the owners hold two windows with that title" $standIn.Both
Check "Win matches the class as well as the title" $standIn.ClassMiss
Check "Close-Settings closes the stand-in's window" $standIn.Exited
Check "Close-Settings leaves an unrelated Cue window open" $standIn.OtherStays
Check "a peak freed before the window shows is counted" ($open.PeakBytes - $open.Stable.PrivateBytes -ge 200MB)
Check "the open time waits for the root's own window" ($open.Ms -ge 1500)

# Starts a copy of PowerShell under a unique name that touches 1.5 GB, waits until it holds it, then stops it with Stop-Cue. Returns whether Stop-Cue threw and how many of that name were still listed after it.
function Stop-BigStandIn {
  $name = "cue-proc-check-" + [guid]::NewGuid().ToString("N").Substring(0, 8)
  $exe = Join-Path $env:TEMP "$name.exe"
  Copy-Item (Join-Path $PSHOME "powershell.exe") $exe
  try {
    $null = Start-Process $exe -ArgumentList '-NoProfile -Command "$a = New-Object byte[] 1500MB; for ($i = 0; $i -lt $a.Length; $i += 4096) { $a[$i] = 1 }; Start-Sleep 120"' -WindowStyle Hidden -PassThru
    $null = Await "the stand-in to touch 1.5 GB" { (Get-Process $name -ErrorAction SilentlyContinue).WorkingSet64 -gt 1400MB } 60000
    $threw = try { Stop-Cue $name; $false } catch { $true }
    [pscustomobject]@{ Threw = $threw; Left = @(Get-Process $name -ErrorAction SilentlyContinue).Count }
  } finally {
    Get-Process $name -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    $null = WaitUntil { -not (Get-Process $name -ErrorAction SilentlyContinue) } 10000
    Remove-Item -LiteralPath $exe -Force
  }
}

$stop = Stop-BigStandIn
Check "Stop-Cue waits until a killed process has left the list" (-not $stop.Threw -and $stop.Left -eq 0)
Complete-Checks
