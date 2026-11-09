# File: scripts/verify/bench.ps1
# Purpose: Cue's performance harness. Launches a release build and measures cold start, idle cost, hotkey-to-overlay latency, memory while recording and the settings window open/close leak check. It reports numbers and never judges them: the fixed hard checks live in lifecycle.ps1, and bench/report.ps1 compares a result with bench/baseline.json.
# Contents: Measure-Startup (one start to hook-ready and its idle window, repeated with -Repeat), Stop-Cue, Idle-Window (an idle measurement that discards windows with keyboard or mouse input), state waits that throw on timeout, the measurement stages, Get-Machine (what the numbers were measured on), the JSON output.
# Usage: pwsh scripts/verify/bench.ps1 [-Exe path] [-Json out.json] [-Dictations 20] [-Cycles 100] [-IdleSeconds 30] [-Repeat 1]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"),
  [string]$Json = (Join-Path $PSScriptRoot "..\..\bench\latest.json"),
  [int]$Dictations = 20,
  [int]$Cycles = 100,
  [int]$IdleSeconds = 30,
  [int]$Repeat = 1,
  [int]$SettleMs = 240000
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
if (-not (Test-Path $Exe)) { throw "Build a release exe first (pnpm tauri build --no-bundle). Not found: $Exe" }
Add-Type -AssemblyName System.Windows.Forms
$null = [Environment]::SetEnvironmentVariable("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", $null)

# Prints what still runs under Cue (processes, windows, log tail) for a run that failed. Input: a label line.
function Dump($label) {
  $label
  $tree = @(Tree $p.Id); $ids = @($tree | ForEach-Object Id)
  Get-CimInstance Win32_Process | Where-Object { $ids -contains $_.ProcessId } | ForEach-Object { "  {0} parent {1} {2}: {3}" -f $_.ProcessId, $_.ParentProcessId, $_.Name, ($_.CommandLine -replace '\s+', ' ').Substring(0, [math]::Min(150, $_.CommandLine.Length)) }
  "windows:"; [Win32]::WindowsOf([uint32[]]$ids) | ForEach-Object { "  $_" }
  $log = Join-Path $env:APPDATA "dev.cue.app\cue.log"; if (Test-Path $log) { "cue.log:"; Get-Content $log -Tail 15 | ForEach-Object { "  $_" } }
}
# Returns what the numbers were measured on (OS build, WebView2 version, machine kind), so a result is only compared with one from the same kind of machine. Returns an ordered map.
function Get-Machine {
  $wv = Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" -ErrorAction SilentlyContinue
  [ordered]@{
    machine = $(if ($env:GITHUB_ACTIONS) { "github-runner" } else { $env:COMPUTERNAME })
    os_build = [Environment]::OSVersion.Version.ToString()
    webview2 = $(if ($wv) { $wv.pv } else { "unknown" })
    cores = [Environment]::ProcessorCount
  }
}
# Waits until the web view's processes have exited and the settings window is gone; returns how long that took. Input: timeout ms.
function Settle($ms) { Await "the settings window and web view processes to exit" { (Snapshot $p.Id).Procs -eq 1 -and (Win $null "Cue") -eq [IntPtr]::Zero } $ms }
# Measures a window in which nobody touched the keyboard or mouse: each key press anywhere wakes Cue's keyboard hook, which is its job and not idle cost, so a window with user input is thrown away and measured again. Inputs: root process id and window length in seconds. Returns the snapshots before and after; throws after five tries so a busy machine never gives a quiet-looking number.
function Idle-Window($rootId, $seconds) {
  for ($try = 1; $try -le 5; $try++) {
    $last = [Win32]::LastInput(); $before = Snapshot $rootId; Start-Sleep $seconds; $after = Snapshot $rootId
    if ([Win32]::LastInput() -eq $last) { return [pscustomobject]@{ A = $before; B = $after } }
    "user input during the idle window, measuring again ($try of 5)"
  }
  throw "The keyboard or mouse was used during every idle window; leave the machine alone while the bench runs."
}
# Waits until private memory stops moving (two samples 0.5 MB apart or less), then returns the sample. Input: timeout ms.
function Quiet($ms) {
  $null = Wait-Stable "private memory to stop moving" { (Snapshot $p.Id).PrivateMB } 0.5 $ms
  Snapshot $p.Id
}
# Presses the default dictation shortcut (Ctrl+Space).
function Down { [Win32]::keybd_event(0xA2, 0x1D, 0, [UIntPtr]::Zero); [Win32]::keybd_event(0x20, 0x39, 0, [UIntPtr]::Zero) }
# Releases the default dictation shortcut.
function Up { [Win32]::keybd_event(0x20, 0x39, 2, [UIntPtr]::Zero); [Win32]::keybd_event(0xA2, 0x1D, 2, [UIntPtr]::Zero) }
# Opens the settings window and closes it again. Inputs: root process id, exe path. Returns open time and the peak snapshot.
function OpenClose($rootId, $exe) { $o = Open-Settings $rootId $exe; Close-Settings; $o }
# Returns whether a speech model is installed (any .onnx file under the models folder); without one a hotkey press shows a notice instead of recording.
function Test-SpeechModel { [bool](Get-ChildItem (Join-Path $env:LOCALAPPDATA "dev.cue.app\models") -Recurse -Filter *.onnx -ErrorAction SilentlyContinue | Select-Object -First 1) }
# Stops every Cue process and waits for them to go.
function Stop-Cue { Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force; $null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue }
# Launches Cue and measures one start: cold start ends when the keyboard hook is ready (the process exists long before, and the tray window earlier than the hook), reported both as wall time from process creation and as the monotonic time the input thread logged; then the idle window with the memory in raw bytes. Sets $script:app to the process. Returns the numbers.
function Measure-Startup {
  $mark = Get-LogMark
  $script:app = Start-Process $Exe -ArgumentList "--background" -PassThru
  $app = $script:app; $script:p = $app
  $hook = Wait-HookReady $mark 20000
  $cold = [int]((Get-Date) - $app.StartTime).TotalMilliseconds
  $null = Await "the thread count to stop changing after start" { $n = (Snapshot $app.Id).Threads; Start-Sleep -Milliseconds 500; $n -eq (Snapshot $app.Id).Threads } 30000
  $w = Idle-Window $app.Id $IdleSeconds
  [pscustomobject]@{ ColdMs = $cold; HookReadyMs = $hook; Before = $w.A; After = $w.B }
}
Stop-Cue
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $settingsBackup = Join-Path $env:TEMP "cue-settings-backup.json"
$moved = $false; $p = $null
if (Test-Path $settings) { Move-Item $settings $settingsBackup -Force; $moved = $true }
try {
  New-Item -Force -ItemType Directory -Path (Split-Path $settings) | Out-Null
  Set-Content -Path $settings -Value '{"onboarded":true,"dictation_mode":"Hold","mic_linger_secs":1}'
  $result = [ordered]@{ schema = 2 }
  foreach ($k in (Get-Machine).Keys) { $result[$k] = (Get-Machine)[$k] }

  $runs = @()
  for ($r = 1; $r -le $Repeat; $r++) {
    $run = Measure-Startup
    $runs += $run
    if ($r -lt $Repeat) { Stop-Cue }
  }
  $p = $script:app
  $result.cold_start_ms = [int](Percentile ($runs | ForEach-Object ColdMs) 50)
  $result.hook_ready_ms = [int](Percentile ($runs | ForEach-Object HookReadyMs) 50)
  $result.idle_private_bytes = [long](Percentile ($runs | ForEach-Object { $_.After.PrivateBytes }) 50)
  $result.idle_private_mb = [math]::Round($result.idle_private_bytes / 1MB, 2)
  $result.idle_working_set_bytes = [long](Percentile ($runs | ForEach-Object { $_.After.WorkingSetBytes }) 50)
  $result.idle_working_set_mb = [math]::Round($result.idle_working_set_bytes / 1MB, 2)
  $result.idle_threads = [int](Percentile ($runs | ForEach-Object { $_.After.Threads }) 50)
  $result.idle_cpu_seconds = [math]::Round((Percentile ($runs | ForEach-Object { $_.After.Cpu - $_.Before.Cpu }) 50), 3)
  $result.idle_window_seconds = $IdleSeconds
  $result.startup_runs = $Repeat
  if ($Repeat -gt 1) {
    $result.cold_start_runs_ms = @($runs | ForEach-Object ColdMs)
    $result.idle_private_runs_bytes = @($runs | ForEach-Object { $_.After.PrivateBytes })
  }

  $skipped = @()
  if (Test-SpeechModel) {
    $show = @(); $hide = @(); $recMem = $null
    for ($i = 0; $i -lt $Dictations; $i++) {
      Down
      $show += Await "the overlay to show after the hotkey" { Visible "CueOverlay" $null } 3000
      if ($i -eq 0) { Start-Sleep -Milliseconds 2000; $recMem = (Quiet 10000).PrivateMB } else { Start-Sleep -Milliseconds 700 }
      $sw = [Diagnostics.Stopwatch]::StartNew(); Up
      $null = Await "the overlay to hide after release" { -not (Visible "CueOverlay" $null) } 20000; $hide += $sw.ElapsedMilliseconds
      Start-Sleep -Milliseconds 300
    }
    $result.overlay_show_p50_ms = Percentile $show 50; $result.overlay_show_p95_ms = Percentile $show 95
    $result.overlay_hide_p50_ms = Percentile $hide 50
    $result.recording_private_mb = $recMem
  } else {
    $skipped = @("overlay_show_p50_ms", "overlay_show_p95_ms", "overlay_hide_p50_ms", "recording_private_mb")
    "SKIPPED: dictation stage, no speech model is installed on this machine (a hotkey press only shows a notice)."
  }
  if ($skipped.Count -eq 0) { Start-Sleep -Milliseconds 2000 }
  $afterDictation = Quiet 30000
  if ($skipped.Count -eq 0) { $result.post_dictation_private_mb = $afterDictation.PrivateMB; $result.post_dictation_threads = $afterDictation.Threads }
  else { $skipped += @("post_dictation_private_mb", "post_dictation_threads") }

  $warm = OpenClose $p.Id $Exe
  $result.settings_open_private_mb = $warm.Stable.PrivateMB; $result.settings_open_procs = $warm.Stable.Procs
  $result.settings_open_private_bytes = $warm.Stable.PrivateBytes; $result.settings_open_peak_private_bytes = $warm.PeakBytes
  $result.settings_open_peak_private_mb = [math]::Round($warm.PeakBytes / 1MB, 2)
  $result.settle_warm_ms = Settle $SettleMs
  $beforeSnap = Quiet 30000; $before = $beforeSnap.PrivateMB; $open = @()
  for ($i = 0; $i -lt $Cycles; $i++) { $open += (OpenClose $p.Id $Exe).Ms }
  $result.settle_end_ms = Settle $SettleMs
  $after = Quiet 30000
  $result.settings_open_p50_ms = Percentile $open 50
  $result.leak_baseline_mb = $before; $result.leak_after_mb = $after.PrivateMB
  $result.leak_baseline_bytes = $beforeSnap.PrivateBytes; $result.leak_after_bytes = $after.PrivateBytes
  $result.leak_delta_mb = [math]::Round($after.PrivateMB - $before, 2)
  $result.procs_after_close = $after.Procs
  $result.after_close_private_mb = $after.PrivateMB; $result.after_close_threads = $after.Threads
  $w = Idle-Window $p.Id $IdleSeconds
  $result.after_close_cpu_seconds = [math]::Round($w.B.Cpu - $w.A.Cpu, 3)

  $result | ConvertTo-Json | Set-Content -Path (New-Item -Force -Path $Json).FullName
  $result.GetEnumerator() | ForEach-Object { "{0,-28} {1}" -f $_.Key, $_.Value }
  foreach ($name in $skipped) { "SKIPPED: $name" }
} catch {
  if ($p) { Dump "bench failed: $($_.Exception.Message)" }
  throw
} finally {
  Up
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  if ($moved) { Move-Item $settingsBackup $settings -Force } else { Remove-Item $settings -Force -ErrorAction SilentlyContinue }
}
