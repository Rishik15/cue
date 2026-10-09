# Purpose: Cue's performance harness. Launches a release build and measures cold start, idle cost, hotkey-to-overlay
# latency, memory while recording, and the settings window open/close leak check; prints a table, writes JSON, and can
# gate against a baseline so a regression fails CI.
# Usage: pwsh scripts/bench.ps1 [-Exe path] [-Json out.json] [-Gate bench/baseline.json] [-MemoryOnly] [-Dictations 20] [-Cycles 100] [-IdleSeconds 30]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"),
  [string]$Json = (Join-Path $PSScriptRoot "..\bench\latest.json"),
  [string]$Gate = "",
  [int]$Dictations = 20,
  [int]$Cycles = 100,
  [int]$IdleSeconds = 30,
  [switch]$MemoryOnly   # gate memory, leaks and idle CPU only (shared CI runners make latency noisy)
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Exe)) { throw "Build a release exe first (pnpm tauri build --no-bundle). Not found: $Exe" }
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public class B {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string cls, string title);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
}
'@
$null = [Environment]::SetEnvironmentVariable("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", $null)

function Tree($rootId) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
  $ids = @($rootId); $grew = $true
  while ($grew) {
    $kids = $all | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object ProcessId
    $grew = [bool]$kids; $ids += $kids
  }
  Get-Process -Id $ids -ErrorAction SilentlyContinue
}
function Snapshot($rootId) {
  $t = Tree $rootId
  [pscustomobject]@{
    Procs = @($t).Count
    PrivateMB = [math]::Round(($t | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB, 1)
    WorkingSetMB = [math]::Round(($t | Measure-Object WorkingSet64 -Sum).Sum / 1MB, 1)
    Threads = ($t | ForEach-Object { $_.Threads.Count } | Measure-Object -Sum).Sum
    Cpu = ($t | Measure-Object CPU -Sum).Sum
  }
}
function Percentile($values, $p) { $s = @($values | Sort-Object); if (-not $s) { return $null }; $s[[math]::Min($s.Count - 1, [int][math]::Ceiling($p / 100 * $s.Count) - 1)] }
# PowerShell turns $null into "" for string parameters, which FindWindow treats as a title to match; pass a real null.
function Win($cls, $title) { [B]::FindWindow($(if ($cls) { $cls } else { [NullString]::Value }), $(if ($title) { $title } else { [NullString]::Value })) }
function Visible($cls, $title) { $h = Win $cls $title; ($h -ne [IntPtr]::Zero) -and [B]::IsWindowVisible($h) }
function WaitUntil($cond, $ms) { $sw = [Diagnostics.Stopwatch]::StartNew(); while (-not (& $cond) -and $sw.ElapsedMilliseconds -lt $ms) { Start-Sleep -Milliseconds 5 }; $sw.ElapsedMilliseconds }

Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 1
$result = [ordered]@{}

# 1. Cold start: launch with no window, wait for the tray's window to exist.
$sw = [Diagnostics.Stopwatch]::StartNew()
$p = Start-Process $Exe -ArgumentList "--background" -PassThru
$null = WaitUntil { [B]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
$result.cold_start_ms = $sw.ElapsedMilliseconds

# 2. Idle: tray only, memory and CPU over the sampling window.
Start-Sleep 8
$a = Snapshot $p.Id; Start-Sleep $IdleSeconds; $b = Snapshot $p.Id
$result.idle_private_mb = $b.PrivateMB; $result.idle_working_set_mb = $b.WorkingSetMB; $result.idle_threads = $b.Threads
$result.idle_cpu_percent = [math]::Round(($b.Cpu - $a.Cpu) / $IdleSeconds * 100, 2)

# 3. Dictation: inject the hotkey, time press -> overlay visible and release -> overlay hidden, sample memory while recording.
function Down { [B]::keybd_event(0xA2, 0x1D, 0, [UIntPtr]::Zero); [B]::keybd_event(0x20, 0x39, 0, [UIntPtr]::Zero) }
function Up { [B]::keybd_event(0x20, 0x39, 2, [UIntPtr]::Zero); [B]::keybd_event(0xA2, 0x1D, 2, [UIntPtr]::Zero) }
$show = @(); $hide = @(); $recMem = @()
for ($i = 0; $i -lt $Dictations; $i++) {
  $sw = [Diagnostics.Stopwatch]::StartNew(); Down
  $null = WaitUntil { Visible "CueOverlay" $null } 3000; $show += $sw.ElapsedMilliseconds
  Start-Sleep -Milliseconds 700; if ($i -eq 0) { $recMem = (Snapshot $p.Id).PrivateMB }
  $sw.Restart(); Up
  if (Visible "CueOverlay" $null) { Down; Up }   # Toggle mode needs a second press to stop
  $null = WaitUntil { -not (Visible "CueOverlay" $null) } 3000; $hide += $sw.ElapsedMilliseconds
  Start-Sleep -Milliseconds 300
}
$result.overlay_show_p50_ms = Percentile $show 50; $result.overlay_show_p95_ms = Percentile $show 95
$result.overlay_hide_p50_ms = Percentile $hide 50
$result.recording_private_mb = $recMem
Start-Sleep 7   # the mic closes after its linger time and memory is trimmed

# 4. Settings window open/close cycles (a second launch makes the running instance open it). The first open loads the web
# view runtime for good (a one-time rise), so the leak baseline is taken after one warm-up cycle.
function OpenClose() {
  $sw = [Diagnostics.Stopwatch]::StartNew(); Start-Process $Exe | Out-Null
  $null = WaitUntil { Visible $null "Cue" } 5000; $ms = $sw.ElapsedMilliseconds
  $peak = Snapshot $p.Id
  # A close posted while the window is still starting up can be dropped, so repeat until it is gone.
  for ($k = 0; $k -lt 6 -and (Visible $null "Cue"); $k++) {
    [B]::PostMessage((Win $null "Cue"), 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    $null = WaitUntil { -not (Visible $null "Cue") } 1500
  }
  [pscustomobject]@{ Ms = $ms; Peak = $peak }
}
$warm = OpenClose
$result.settings_open_private_mb = $warm.Peak.PrivateMB; $result.settings_open_procs = $warm.Peak.Procs
$null = WaitUntil { (Snapshot $p.Id).Procs -eq 1 } 20000   # the web view processes exit shortly after the window is destroyed
Start-Sleep 6
$before = (Snapshot $p.Id).PrivateMB; $open = @()
for ($i = 0; $i -lt $Cycles; $i++) { $open += (OpenClose).Ms }
$null = WaitUntil { (Snapshot $p.Id).Procs -eq 1 } 20000
Start-Sleep 6
$after = Snapshot $p.Id
$result.settings_open_p50_ms = Percentile $open 50
$result.leak_baseline_mb = $before; $result.leak_after_mb = $after.PrivateMB
$result.leak_delta_mb = [math]::Round($after.PrivateMB - $before, 1)
$result.procs_after_close = $after.Procs
$p | Stop-Process -Force

$result | ConvertTo-Json | Set-Content -Path (New-Item -Force -Path $Json).FullName
$result.GetEnumerator() | ForEach-Object { "{0,-28} {1}" -f $_.Key, $_.Value }

# Gate: fail when a number regresses past its baseline (memory +5 %, latency +10 %, leak over 4 MB per 100 cycles, idle CPU over 1 %).
if ($Gate) {
  $base = Get-Content $Gate -Raw | ConvertFrom-Json; $bad = @()
  function Over($key, $factor, $slack) { if ($result[$key] -gt $base.$key * $factor + $slack) { $script:bad += "$key $($result[$key]) > baseline $($base.$key)" } }
  Over "idle_private_mb" 1.05 0.5; Over "settings_open_private_mb" 1.05 5
  if (-not $MemoryOnly) { Over "cold_start_ms" 1.10 50; Over "overlay_show_p50_ms" 1.10 10; Over "settings_open_p50_ms" 1.10 100 }
  if ($result.leak_delta_mb -gt 4) { $bad += "leak_delta_mb $($result.leak_delta_mb) > 4" }
  if ($result.idle_cpu_percent -gt 1) { $bad += "idle_cpu_percent $($result.idle_cpu_percent) > 1" }
  if ($bad) { "GATE FAILED:"; $bad; exit 1 } else { "GATE PASSED" }
}
