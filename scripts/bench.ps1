# Purpose: Cue's performance harness. Launches a release build and measures cold start, idle cost, hotkey-to-overlay
# latency, memory while recording, and the settings window open/close leak check; prints a table, writes JSON, and can
# gate against the budgets in bench/budget.json so going over one fails CI.
# Usage: pwsh scripts/bench.ps1 [-Exe path] [-Json out.json] [-Gate bench/budget.json] [-Dictations 20] [-Cycles 100] [-IdleSeconds 30]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"),
  [string]$Json = (Join-Path $PSScriptRoot "..\bench\latest.json"),
  [string]$Gate = "",
  [int]$Dictations = 20,
  [int]$Cycles = 100,
  [int]$IdleSeconds = 30,
  # How long the web view's processes get to exit after the window is gone. WebView2 ends them on its own schedule (under a second on one run, over a minute on the next), so this is patience, not a budget.
  [int]$SettleMs = 240000
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
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  // Every top-level window the given processes own: process, class, title, visible.
  public static System.Collections.Generic.List<string> WindowsOf(uint[] pids) {
    var list = new System.Collections.Generic.List<string>();
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (Array.IndexOf(pids, pid) >= 0) {
        var c = new System.Text.StringBuilder(128); var t = new System.Text.StringBuilder(128);
        GetClassName(h, c, 128); GetWindowText(h, t, 128);
        list.Add(pid + " " + c + " \"" + t + "\" visible=" + IsWindowVisible(h));
      }
      return true;
    }, IntPtr.Zero);
    return list;
  }
}
'@
$null = [Environment]::SetEnvironmentVariable("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", $null)

function Tree($rootId) {
  # The speech worker is a separate budget (it exists only while dictating and holds the model), so it is left out of the tree.
  $all = Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -notmatch "--speech-worker" } | Select-Object ProcessId, ParentProcessId
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
# Waits until the settings window is gone and its web view processes have exited; returns how long that took.
function Settle($ms) { WaitUntil { (Snapshot $p.Id).Procs -eq 1 -and (Win $null "Cue") -eq [IntPtr]::Zero } $ms }
# What is still running under Cue, for a run that did not settle (which of the web view's processes lingered, and for how long).
function Dump($label) {
  $label
  $tree = @(Tree $p.Id); $ids = @($tree | ForEach-Object Id)
  Get-CimInstance Win32_Process | Where-Object { $ids -contains $_.ProcessId } | ForEach-Object { "  {0} parent {1} {2}: {3}" -f $_.ProcessId, $_.ParentProcessId, $_.Name, ($_.CommandLine -replace '\s+', ' ').Substring(0, [math]::Min(150, $_.CommandLine.Length)) }
  "windows:"; [B]::WindowsOf([uint32[]]$ids) | ForEach-Object { "  $_" }
  $log = Join-Path $env:APPDATA "dev.cue.app\cue.log"; if (Test-Path $log) { "cue.log:"; Get-Content $log -Tail 15 | ForEach-Object { "  $_" } }
}
function Percentile($values, $p) { $s = @($values | Sort-Object); if (-not $s) { return $null }; $s[[math]::Min($s.Count - 1, [int][math]::Ceiling($p / 100 * $s.Count) - 1)] }
# PowerShell turns $null into "" for string parameters, which FindWindow treats as a title to match; pass a real null.
function Win($cls, $title) { [B]::FindWindow($(if ($cls) { $cls } else { [NullString]::Value }), $(if ($title) { $title } else { [NullString]::Value })) }
function Visible($cls, $title) { $h = Win $cls $title; ($h -ne [IntPtr]::Zero) -and [B]::IsWindowVisible($h) }
function WaitUntil($cond, $ms) { $sw = [Diagnostics.Stopwatch]::StartNew(); while (-not (& $cond) -and $sw.ElapsedMilliseconds -lt $ms) { Start-Sleep -Milliseconds 5 }; $sw.ElapsedMilliseconds }

Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 1
# The dictation step presses the default shortcut, so the user's settings are set aside while it runs and put back after (as smoke.ps1 does).
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $settingsBackup = Join-Path $env:TEMP "cue-settings-backup.json"
if (Test-Path $settings) { Move-Item $settings $settingsBackup -Force }
# A first run opens the walkthrough window, which is not what Cue costs day to day: start as an app that has been set up.
New-Item -Force -ItemType Directory -Path (Split-Path $settings) | Out-Null
Set-Content -Path $settings -Value '{"onboarded":true}'
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
  # The window is created hidden and shows itself once its page has loaded, which can take several seconds on a busy runner.
  $null = WaitUntil { Visible $null "Cue" } 30000; $ms = $sw.ElapsedMilliseconds
  if (-not (Visible $null "Cue")) { Dump "the settings window did not open"; throw "The settings window was not shown 30 s after it was asked for." }
  # A person closes a window they can see, not one still being built: wait until the web view has started and stopped growing.
  $null = WaitUntil { (Snapshot $p.Id).Procs -gt 1 } 5000
  do { $n = (Snapshot $p.Id).Procs; Start-Sleep -Milliseconds 400 } while ((Snapshot $p.Id).Procs -ne $n)
  $peak = Snapshot $p.Id
  # Ask it to close until the window itself is gone (not just hidden): a request sent while the page is busy is dropped, so ask again.
  $sw.Restart()
  while ((Win $null "Cue") -ne [IntPtr]::Zero -and $sw.ElapsedMilliseconds -lt 30000) {
    [B]::PostMessage((Win $null "Cue"), 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    $null = WaitUntil { (Win $null "Cue") -eq [IntPtr]::Zero } 1500
  }
  if ((Win $null "Cue") -ne [IntPtr]::Zero) { Dump "the settings window did not close"; throw "The settings window was still there 30 s after being asked to close." }
  [pscustomobject]@{ Ms = $ms; Peak = $peak }
}
$warm = OpenClose
$result.settings_open_private_mb = $warm.Peak.PrivateMB; $result.settings_open_procs = $warm.Peak.Procs
$result.settle_warm_ms = Settle $SettleMs
if ((Snapshot $p.Id).Procs -ne 1) { Dump "warm-up did not settle"; throw "The web view's processes were still running $($SettleMs / 1000) s after the first close, so the leak check has no clean baseline." }
Start-Sleep 6
$before = (Snapshot $p.Id).PrivateMB; $open = @()
for ($i = 0; $i -lt $Cycles; $i++) { $open += (OpenClose).Ms }
$result.settle_end_ms = Settle $SettleMs
if ((Snapshot $p.Id).Procs -ne 1) { Dump "last cycle did not settle" }
Start-Sleep 6
$after = Snapshot $p.Id
$result.settings_open_p50_ms = Percentile $open 50
$result.leak_baseline_mb = $before; $result.leak_after_mb = $after.PrivateMB
$result.leak_delta_mb = [math]::Round($after.PrivateMB - $before, 1)
$result.procs_after_close = $after.Procs
$p | Stop-Process -Force
if (Test-Path $settingsBackup) { Move-Item $settingsBackup $settings -Force } else { Remove-Item $settings -Force -ErrorAction SilentlyContinue }

$result | ConvertTo-Json | Set-Content -Path (New-Item -Force -Path $Json).FullName
$result.GetEnumerator() | ForEach-Object { "{0,-28} {1}" -f $_.Key, $_.Value }

# Gate: fail when a number is over its budget (bench/budget.json: limits that hold for every version, not "no worse than before").
if ($Gate) {
  $budget = Get-Content $Gate -Raw | ConvertFrom-Json; $bad = @()
  foreach ($limit in $budget.PSObject.Properties) { if ($result[$limit.Name] -gt $limit.Value) { $bad += "$($limit.Name) $($result[$limit.Name]) > budget $($limit.Value)" } }
  if ($bad) { "GATE FAILED:"; $bad; exit 1 } else { "GATE PASSED" }
}
