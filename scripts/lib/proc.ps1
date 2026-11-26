# File: scripts/lib/proc.ps1
# Purpose: process-tree, window and stop helpers shared by the harnesses, so every one measures the same tree the same way and stops Cue the same way.
# Contents: Tree (the processes under a root, without the speech worker), Snapshot (count, memory, threads and CPU seconds of that tree), Percentile, Win and Visible (top-level window lookups), Dump-Failure (what still runs under a failed run), Open-Settings and Close-Settings (the settings window round trip), Stop-Cue (end every Cue process and make sure they are gone).
# Usage: . "$PSScriptRoot\..\lib\proc.ps1" after win32.ps1 is loaded.

# Returns the process tree under a root id, leaving out the speech worker unless asked (the bench measures it on its own; lifecycle must see it). Inputs: root process id, whether to keep the worker.
function Tree($rootId, [bool]$withWorker = $false) {
  $all = Get-CimInstance Win32_Process | Where-Object { $withWorker -or $_.CommandLine -notmatch "--speech-worker" } | Select-Object ProcessId, ParentProcessId
  $ids = @($rootId); $grew = $true
  while ($grew) {
    $kids = $all | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object ProcessId
    $grew = [bool]$kids; $ids += $kids
  }
  Get-Process -Id $ids -ErrorAction SilentlyContinue
}

# Returns process count, memory, thread count and CPU seconds summed over the tree. Memory comes as raw bytes (PrivateBytes, WorkingSetBytes, PeakCommitBytes: the sum of what the system recorded as each process's peak commit) and as MB to two decimals for display. Inputs: root process id, whether to keep the speech worker.
function Snapshot($rootId, [bool]$withWorker = $false) {
  $t = Tree $rootId $withWorker
  $private = [long](($t | Measure-Object PrivateMemorySize64 -Sum).Sum); $working = [long](($t | Measure-Object WorkingSet64 -Sum).Sum)
  [pscustomobject]@{
    Procs = @($t).Count
    PrivateBytes = $private
    WorkingSetBytes = $working
    PeakCommitBytes = [long](($t | Measure-Object PeakPagedMemorySize64 -Sum).Sum)
    PrivateMB = [math]::Round($private / 1MB, 2)
    WorkingSetMB = [math]::Round($working / 1MB, 2)
    Threads = ($t | ForEach-Object { $_.Threads.Count } | Measure-Object -Sum).Sum
    Cpu = ($t | Measure-Object CPU -Sum).Sum
  }
}

# Returns the pct-th percentile of a list, or $null when it is empty. Inputs: values, percentile 0-100.
function Percentile($values, $pct) { $s = @($values | Sort-Object); if (-not $s) { return $null }; $s[[math]::Min($s.Count - 1, [int][math]::Ceiling($pct / 100 * $s.Count) - 1)] }

# Finds a top-level window by class and/or title; PowerShell turns $null into "" for string parameters, so a real null is passed. Inputs: class, title.
function Win($cls, $title) { [Win32]::FindWindow($(if ($cls) { $cls } else { [NullString]::Value }), $(if ($title) { $title } else { [NullString]::Value })) }

# Returns whether the window exists and is visible. Inputs: class, title.
function Visible($cls, $title) { $h = Win $cls $title; ($h -ne [IntPtr]::Zero) -and [Win32]::IsWindowVisible($h) }

# Samples the tree while it waits and remembers the largest private byte count seen; the real peak of a window is the larger of that and the system's own peak commit of the processes still alive (a short spike between samples is caught by the system's figure). Input: root process id, whether to count the speech worker, the running maximum. Returns the new maximum.
function Sample-Peak($rootId, [bool]$withWorker, [long]$seen) { [math]::Max($seen, (Snapshot $rootId $withWorker).PrivateBytes) }

# Prints what still runs under Cue (processes, windows, log tail) for a run that failed. Inputs: a label line, the root process id.
function Dump-Failure($label, $rootId) {
  $label
  $ids = @(Tree $rootId | ForEach-Object Id)
  Get-CimInstance Win32_Process | Where-Object { $ids -contains $_.ProcessId } | ForEach-Object { "  {0} parent {1} {2}: {3}" -f $_.ProcessId, $_.ParentProcessId, $_.Name, ($_.CommandLine -replace '\s+', ' ').Substring(0, [math]::Min(150, $_.CommandLine.Length)) }
  "windows:"; [Win32]::WindowsOf([uint32[]]$ids) | ForEach-Object { "  $_" }
  $log = Join-Path $env:APPDATA "dev.cue.app\cue.log"; if (Test-Path $log) { "cue.log:"; Get-Content $log -Tail 15 | ForEach-Object { "  $_" } }
}

# shortcut: PeakBytes sums the surviving processes' lifetime peaks, so an earlier allocation stays charged and peaks at different times add up; upgrade to a per-interval tree peak sampled from the job object when a number from it is used to judge a change.
# Opens the settings window with a second launch and waits until it is shown and its web view processes stop changing, sampling memory while the web view settles (the open time is measured before any sampling, so it is not slowed by it). Inputs: root process id, exe path, whether to count the speech worker. Returns open time in ms, the settled snapshot (Stable) and the peak private bytes seen while opening (PeakBytes, from sampling and the system's peak commit); throws on a timeout. The wait for the window is 120 s because the first open on a cold web view profile builds its environment with no timeout: measured on a loaded windows-latest runner (150 fresh-profile opens, 4 busy loops) the build took p50 5.7 s, p95 15.2 s, max 21.2 s, and unloaded the first open reached 14 s; warm opens take 0.3-0.4 s.
function Open-Settings($rootId, $exe, [bool]$withWorker = $false) {
  Start-Process $exe | Out-Null
  $script:seenPeak = [long]0
  $ms = Await "the settings window to be shown" { Visible $null "Cue" } 120000
  $null = Await "the web view to start" { (Snapshot $rootId $withWorker).Procs -gt 1 } 5000
  $script:procCount = -1
  $null = Await "the web view process count to stop changing" { $now = (Snapshot $rootId $withWorker).Procs; $script:seenPeak = Sample-Peak $rootId $withWorker $script:seenPeak; $same = $now -eq $script:procCount; $script:procCount = $now; Start-Sleep -Milliseconds 400; $same } 30000
  $stable = Snapshot $rootId $withWorker
  [pscustomobject]@{ Ms = $ms; Stable = $stable; PeakBytes = [math]::Max($script:seenPeak, $stable.PeakCommitBytes) }
}

# Asks the settings window to close until it is gone; throws when it is still there after 30 s.
function Close-Settings {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ((Win $null "Cue") -ne [IntPtr]::Zero -and $sw.ElapsedMilliseconds -lt 30000) {
    [Win32]::PostMessage((Win $null "Cue"), 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    $null = WaitUntil { (Win $null "Cue") -eq [IntPtr]::Zero } 1500
  }
  if ((Win $null "Cue") -ne [IntPtr]::Zero) { throw "The settings window was still there 30 s after being asked to close." }
}

# Stops every process of that name and waits up to 10 s for them to be gone; throws "Cue did not exit within 10 s" when one remains. Input: process name (the self-checks pass one no process has).
function Stop-Cue($name = "cue") {
  Get-Process $name -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Get-Process $name -ErrorAction SilentlyContinue | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
  if (Get-Process $name -ErrorAction SilentlyContinue) { throw "Cue did not exit within 10 s" }
}
