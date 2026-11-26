# File: scripts/test/proc.check.ps1
# Purpose: self-check of scripts/lib/proc.ps1: Percentile, which every bench number goes through (the nearest-rank median and 95th percentile of 1..10, and $null for an empty list), and the peak sampler failing loudly when a sample fails, Open-Settings counting a memory peak that comes and goes before the window shows (a stand-in process commits 300 MB, frees it, then shows a window titled "Cue"), and Stop-Cue waiting out a killed process that stays listed after it has exited (a stand-in with 1.5 GB touched takes 100-200 ms to leave the list).
# Contents: the Percentile cases, the failed-sample cases, Open-StandIn (the stand-in launch through Open-Settings), the peak cases, Stop-BigStandIn and the Stop-Cue case, the verdict.
# Usage: & scripts/test/proc.check.ps1   (cue.ps1 test runs it; it refuses to run while a window titled "Cue" is open)
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

# Writes the stand-in to a temp folder, opens it through Open-Settings as a child of this process, closes its window and waits until it has exited. Returns what Open-Settings returned.
function Open-StandIn {
  $folder = Join-Path $env:TEMP ("cue-proc-check-" + [guid]::NewGuid())
  New-Item -ItemType Directory -Path $folder | Out-Null
  try {
    Set-Content (Join-Path $folder "standin.ps1") 'Add-Type -AssemblyName System.Windows.Forms; $m = [Runtime.InteropServices.Marshal]::AllocHGlobal(300MB); Start-Sleep -Milliseconds 1500; [Runtime.InteropServices.Marshal]::FreeHGlobal($m); $f = New-Object Windows.Forms.Form; $f.Text = "Cue"; [Windows.Forms.Application]::Run($f)'
    Set-Content (Join-Path $folder "standin.cmd") '@powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0standin.ps1"'
    $before = @(Tree $PID).Count
    $open = Open-Settings $PID (Join-Path $folder "standin.cmd")
    Close-Settings
    $null = Await "the stand-in to exit" { @(Tree $PID).Count -le $before } 10000
    $open
  } finally { Remove-Item -LiteralPath $folder -Recurse -Force }
}

if ((Win $null "Cue") -ne [IntPtr]::Zero) { throw "Close the window titled Cue (Cue's settings) before running this check." }
$open = Open-StandIn
Check "a peak freed before the window shows is counted" ($open.PeakBytes - $open.Stable.PrivateBytes -ge 200MB)
Check "the open time runs from the launch to the window" ($open.Ms -ge 1500)

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
