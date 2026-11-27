# File: scripts/lib/proc.ps1
# Purpose: process-tree, window and stop helpers shared by the harnesses, so every one measures the same tree the same way and stops Cue the same way.
# Contents: Tree (the processes under a root, without the speech worker unless asked), Snapshot (count, memory, threads and CPU seconds of that tree), Percentile, Win and Visible (top-level window lookups), SettingsWindow (the settings window of one Cue), Start-PeakSampler and Stop-PeakSampler (the largest private byte count of the tree, sampled on a background runspace), Dump-Failure (what still runs under a failed run), Open-Settings and Close-Settings (the settings window round trip), Stop-Cue (end every Cue process and make sure they are gone).
# Usage: . "$PSScriptRoot\..\lib\proc.ps1" after win32.ps1 is loaded.

# Returns the process tree under a root id, leaving out the speech worker unless asked (the bench reports the worker separately through `Snapshot $id $true`; lifecycle must see it). Inputs: root process id, whether to keep the worker.
function Tree($rootId, [bool]$withWorker = $false) {
  $all = Get-CimInstance Win32_Process | Where-Object { $withWorker -or $_.CommandLine -notmatch "--speech-worker" } | Select-Object ProcessId, ParentProcessId
  $ids = @($rootId); $grew = $true
  while ($grew) {
    $kids = $all | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object ProcessId
    $grew = [bool]$kids; $ids += $kids
  }
  Get-Process -Id $ids -ErrorAction SilentlyContinue
}

# Returns process count, memory, thread count and CPU seconds summed over the tree. Memory comes as raw bytes (PrivateBytes, WorkingSetBytes) and as MB to two decimals for display. Inputs: root process id, whether to keep the speech worker.
function Snapshot($rootId, [bool]$withWorker = $false) {
  $t = Tree $rootId $withWorker
  $private = [long](($t | Measure-Object PrivateMemorySize64 -Sum).Sum); $working = [long](($t | Measure-Object WorkingSet64 -Sum).Sum)
  [pscustomobject]@{
    Procs = @($t).Count
    PrivateBytes = $private
    WorkingSetBytes = $working
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

# Returns the settings window: the top-level window titled "Cue" that the root process owns, so a user's window with that title (an Explorer folder named Cue) is never timed or closed. Input: root process id. Returns the handle, or zero.
function SettingsWindow($rootId) { [Win32]::WindowOf([uint32]$rootId, "Cue") }

# Starts sampling the tree's private bytes on a background runspace (one tree query, then 100 ms, again until stopped), so the sampling never delays the caller's own timing; any error in a sample stops the sampler. Inputs: root process id, whether to count the speech worker. Returns the sampler for Stop-PeakSampler.
function Start-PeakSampler($rootId, [bool]$withWorker) {
  $state = [hashtable]::Synchronized(@{ Run = $true; Peak = [long]0 })
  $ps = [powershell]::Create().AddScript({
    param($lib, $id, $w, $s)
    $ErrorActionPreference = "Stop"
    . $lib
    while ($s.Run) { $s.Peak = [math]::Max($s.Peak, (Snapshot $id $w).PrivateBytes); Start-Sleep -Milliseconds 100 }
  }).AddArgument($PSCommandPath).AddArgument($rootId).AddArgument($withWorker).AddArgument($state)
  [pscustomobject]@{ Ps = $ps; Handle = $ps.BeginInvoke(); State = $state }
}

# Stops a sampler from Start-PeakSampler, waits for its last sample and frees it; an error that stopped the sampler is rethrown. Input: the sampler. Returns the largest private byte count it saw.
function Stop-PeakSampler($sampler) {
  $sampler.State.Run = $false
  try { $null = $sampler.Ps.EndInvoke($sampler.Handle) } finally { $sampler.Ps.Dispose() }
  [long]$sampler.State.Peak
}

# Prints what still runs under Cue (processes, windows, log tail) for a run that failed. Inputs: a label line, the root process id.
function Dump-Failure($label, $rootId) {
  $label
  $ids = @(Tree $rootId | ForEach-Object Id)
  Get-CimInstance Win32_Process | Where-Object { $ids -contains $_.ProcessId } | ForEach-Object { "  {0} parent {1} {2}: {3}" -f $_.ProcessId, $_.ParentProcessId, $_.Name, ("$($_.CommandLine)" -replace '\s+', ' ' -replace '^(.{150}).+$', '$1') }
  "windows:"; [Win32]::WindowsOf([uint32[]]$ids) | ForEach-Object { "  $_" }
  $log = Join-Path $env:APPDATA "dev.cue.app\cue.log"; if (Test-Path $log) { "cue.log:"; Get-Content $log -Tail 15 | ForEach-Object { "  $_" } }
}

# shortcut: the settings peak is the largest sample taken from the open request until the web view settles (one tree query plus 100 ms apart), so a spike between two samples is missed; upgrade to the peak of a job object created for the interval when a number from it is used to judge a change.
# Opens the settings window with a second launch and waits until it is shown and its web view processes stop changing, while a background sampler records memory from the open request on (the hidden web view initialises before the window shows, and its peak counts). The open time runs from the launch to the window being visible, as before. Inputs: root process id, exe path, whether to count the speech worker. Returns open time in ms, the settled snapshot (Stable) and the largest private byte count sampled while opening (PeakBytes); throws on a timeout. The wait for the window is 120 s because the first open on a cold web view profile builds its environment with no timeout: measured on a loaded windows-latest runner (150 fresh-profile opens, 4 busy loops) the build took p50 5.7 s, p95 15.2 s, max 21.2 s, and unloaded the first open reached 14 s; warm opens take 0.3-0.4 s.
function Open-Settings($rootId, $exe, [bool]$withWorker = $false) {
  $sampler = Start-PeakSampler $rootId $withWorker
  try {
    Start-Process $exe | Out-Null
    $ms = Await "the settings window to be shown" { $h = SettingsWindow $rootId; $h -ne [IntPtr]::Zero -and [Win32]::IsWindowVisible($h) } 120000
    $null = Await "the web view to start" { (Snapshot $rootId $withWorker).Procs -gt 1 } 5000
    $script:procCount = -1
    $null = Await "the web view process count to stop changing" { $now = (Snapshot $rootId $withWorker).Procs; $same = $now -eq $script:procCount; $script:procCount = $now; Start-Sleep -Milliseconds 400; $same } 30000
    $stable = Snapshot $rootId $withWorker
  } finally { $peak = Stop-PeakSampler $sampler }
  [pscustomobject]@{ Ms = $ms; Stable = $stable; PeakBytes = [math]::Max($peak, $stable.PrivateBytes) }
}

# Asks the root's settings window to close until it is gone; throws when it is still there after 30 s. Input: root process id.
function Close-Settings($rootId) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ((SettingsWindow $rootId) -ne [IntPtr]::Zero -and $sw.ElapsedMilliseconds -lt 30000) {
    [Win32]::PostMessage((SettingsWindow $rootId), 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    $null = WaitUntil { (SettingsWindow $rootId) -eq [IntPtr]::Zero } 1500
  }
  if ((SettingsWindow $rootId) -ne [IntPtr]::Zero) { throw "The settings window was still there 30 s after being asked to close." }
}

# Stops every process of that name and waits up to 10 s until none is listed; throws "Cue did not exit within 10 s" when one remains. It polls the list instead of Wait-Process, which returns once a killed process has exited while Windows still lists it until its memory is torn down (100-200 ms for 1.5 GB). Input: process name (the self-checks pass one no process has).
function Stop-Cue($name = "cue") {
  Get-Process $name -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while (Get-Process $name -ErrorAction SilentlyContinue) {
    if ($sw.ElapsedMilliseconds -ge 10000) { throw "Cue did not exit within 10 s" }
    Start-Sleep -Milliseconds 50
  }
}
