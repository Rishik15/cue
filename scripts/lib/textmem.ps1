# File: scripts/lib/textmem.ps1
# Purpose: memory of a model server as a user would pay for it: the private bytes of the whole process tree and the dedicated GPU memory of those processes, so a child process or video memory cannot hide.
# Contents: TreeIds — a process and all its descendants; TreeMemoryMB — private bytes of that tree in MB; GpuMemoryMB — dedicated GPU memory of that tree in MB (0 when the counters are unavailable).
# Usage: . (Join-Path $PSScriptRoot "..\lib\textmem.ps1")

# Returns the ids of a process and every descendant. Input: the root process id.
function TreeIds([int]$rootId) {
  $children = @{}
  Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId | ForEach-Object { $children[[int]$_.ParentProcessId] = @($children[[int]$_.ParentProcessId]) + [int]$_.ProcessId }
  $ids = @($rootId); $i = 0
  while ($i -lt $ids.Count) { foreach ($c in @($children[[int]$ids[$i]])) { if ($c -and $ids -notcontains $c) { $ids += $c } }; $i++ }
  $ids
}
# Returns the private bytes of a process tree in MB. Input: the root process id.
function TreeMemoryMB([int]$rootId) {
  $sum = 0
  foreach ($id in (TreeIds $rootId)) { $proc = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($proc) { $sum += $proc.PrivateMemorySize64 } }
  [math]::Round($sum / 1MB)
}
# Returns the dedicated GPU memory of a process tree in MB, or 0 when Windows has no counter for it. Input: the root process id.
function GpuMemoryMB([int]$rootId) {
  $sum = 0
  foreach ($id in (TreeIds $rootId)) {
    try { $sum += (Get-Counter "\GPU Process Memory(pid_${id}_*)\Dedicated Usage" -ErrorAction Stop).CounterSamples | Measure-Object -Property CookedValue -Sum | Select-Object -ExpandProperty Sum } catch { }
  }
  [math]::Round($sum / 1MB)
}
