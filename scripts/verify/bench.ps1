# File: scripts/verify/bench.ps1
# Purpose: Cue's performance harness. Launches a release build and measures cold start, idle cost, snippet expansion latency (only with -Expansions, see below), hotkey-to-overlay latency, memory while recording (Cue alone and with its speech worker) and that the worker exits after its unload delay, the cold first and warm settings opens with the open/close leak check (private bytes, and the Process handles Cue still holds after the cycles), and the exe, frontend and installer sizes. It reports numbers and never judges them: the fixed hard checks live in lifecycle.ps1, and bench-report.ps1 compares a result with bench/baseline.json.
# The expansion stage is manual: the typing watcher reads Raw Input from a real keyboard and ignores injected keys (no device), so no script or CI runner can type a trigger. With -Expansions N, Cue runs over the seeded library bench/snippets.db (copied into the data folder Enter-UserData emptied, so Exit-UserData deletes it and puts the user's library back) and a person types the triggers into the Notepad window it opens; without it the rows are SKIPPED and the user's library is never touched.
# Contents: Measure-Startup (one start to hook-ready and its idle window, repeated with -Repeat), Idle-Window (an idle measurement that discards windows with keyboard or mouse input), Expansion-Marks / Measure-Expansions / Measure-ExpansionStage (triggers a person types into a Notepad the bench starts itself, refusing when one is already open, and the timing Cue logged for each), Get-DictationModel (the installed speech model the dictation stage uses), state waits that throw on timeout, Measure-Dictations, Add-Startup / Add-Dictation / Add-SettingsLeak / Add-Sizes (each stage writes its rows into the result and names the rows it skipped in $skipped), Write-Result (the JSON file and the printed rows), Get-Machine (what the numbers were measured on). The result carries its recipe: idle_window_seconds, cycles, dictations and the commit.
# Usage: pwsh scripts/verify/bench.ps1 [-Exe path] [-Json out.json] [-Expansions 0] [-Dictations 20] [-Cycles 100] [-IdleSeconds 30] [-Repeat 1]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"),
  [string]$Json = (Join-Path $PSScriptRoot "..\..\bench\latest.json"),
  [int]$Expansions = 0,
  [int]$Dictations = 20,
  [int]$Cycles = 100,
  [int]$IdleSeconds = 30,
  [int]$Repeat = 1,
  [int]$SettleMs = 240000
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\keys.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
. (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
. (Join-Path $PSScriptRoot "..\lib\notepad.ps1")
. (Join-Path $PSScriptRoot "..\lib\cuelab.ps1")
. (Join-Path $PSScriptRoot "..\lib\handles.ps1")
if (-not (Test-Path $Exe)) { throw "Build a release exe first (pnpm tauri build --no-bundle). Not found: $Exe" }
Add-Type -AssemblyName System.Windows.Forms
$null = [Environment]::SetEnvironmentVariable("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", $null)

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
function Settle($ms) { Await "the settings window and web view processes to exit" { (Snapshot $p.Id).Procs -eq 1 -and (SettingsWindow $p.Id) -eq [IntPtr]::Zero } $ms }
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
# Opens the settings window and closes it again. Inputs: root process id, exe path. Returns open time and the peak snapshot.
function OpenClose($rootId, $exe) { $o = Open-Settings $rootId $exe; Close-Settings $rootId; $o }
$models = Join-Path $env:LOCALAPPDATA "dev.cue.app\models"
# Returns the id of the first speech model whose files are all present at catalog size, by the app's own rule (`cue --lab installed`), or $null when none is or the folder does not exist; the shared VAD alone installs nothing, and without a model a hotkey press shows a notice instead of recording.
function Get-DictationModel {
  if (-not (Test-Path -LiteralPath $models -PathType Container)) { return $null }
  @(Get-CueInstalled $Exe $models) | Select-Object -First 1
}
# Returns the timing marks Cue logged after `$mark` (see Get-LogMark), one per expansion. Input: the log mark. Returns objects with the text mark and the total of the four marks (start, checks, text, typing) in ms.
function Expansion-Marks($mark) {
  foreach ($m in [regex]::Matches((Read-LogSince $mark), "expansion: start (\d+) ms, checks (\d+) ms, text (\d+) ms, typing (\d+) ms")) {
    $ms = foreach ($i in 1..4) { [int]$m.Groups[$i].Value }
    [pscustomobject]@{ Text = $ms[2]; Total = ($ms | Measure-Object -Sum).Sum }
  }
}
# Asks the person at the keyboard to type a trigger `$count` times into the Notepad window (injected keys never reach the watcher) and waits for each expansion Cue logs; one that does not come within 60 s throws. Inputs: the trigger and the count. Returns the timing marks of those expansions.
function Measure-Expansions($trigger, $count) {
  $mark = Get-LogMark
  "TYPE NOW: in the Notepad window, type $trigger then Enter, $count times."
  for ($i = 1; $i -le $count; $i++) {
    $null = Await "Cue to expand $trigger ($i of $count)" { @(Expansion-Marks $mark).Count -ge $i } 60000
  }
  Expansion-Marks $mark
}
# Opens Notepad on a scratch file with a per-run name, deleted once its document is closed (throws when the user has a Notepad open), has the person type the plain trigger and the include trigger `$count` times each, and returns the expansion rows. Input: the count. Returns an ordered map of the six snippet_* rows.
function Measure-ExpansionStage($count) {
  $null = Await "snippet expansion to start over the seeded library" { (Read-LogSince $script:launchMark) -match "snippet expansion started" } 10000
  $part = "cue-bench-expand-" + [guid]::NewGuid()
  $script:notepadFile = Join-Path $env:TEMP "$part.txt"; Set-Content $script:notepadFile ""
  $script:notepad = Start-OwnedNotepad $script:notepadFile
  if ($null -eq $script:notepad) { throw "Notepad is already running; close it so this check cannot touch your documents." }
  $null = Await "Notepad to take the focus" { (New-Object -ComObject WScript.Shell).AppActivate($part) } 5000
  $plain = Measure-Expansions ":sig" $count
  $include = Measure-Expansions ":hi" $count
  Stop-OwnedNotepad $script:notepad $script:notepadFile; $script:notepad = $null; Remove-Item -LiteralPath $script:notepadFile
  [ordered]@{
    snippet_expansion_text_p50_ms = Percentile ($plain | ForEach-Object Text) 50
    snippet_expansion_text_p95_ms = Percentile ($plain | ForEach-Object Text) 95
    snippet_expansion_total_p50_ms = Percentile ($plain | ForEach-Object Total) 50
    snippet_expansion_total_p95_ms = Percentile ($plain | ForEach-Object Total) 95
    snippet_include_text_p50_ms = Percentile ($include | ForEach-Object Text) 50
    snippet_include_total_p50_ms = Percentile ($include | ForEach-Object Total) 50
  }
}
# Launches Cue and measures one start: cold start ends when the keyboard hook is ready (the process exists long before, and the tray window earlier than the hook), reported both as wall time from process creation and as the monotonic time the input thread logged; then the idle window with the memory in raw bytes. Sets $script:app to the process. Returns the numbers.
function Measure-Startup {
  $mark = Get-LogMark; $script:launchMark = $mark
  $script:app = Start-Process $Exe -ArgumentList "--background" -PassThru
  $app = $script:app; $script:p = $app
  $hook = Wait-HookReady $mark 20000
  $cold = [int]((Get-Date) - $app.StartTime).TotalMilliseconds
  $null = Await "the thread count to stop changing after start" { $n = (Snapshot $app.Id).Threads; Start-Sleep -Milliseconds 500; $n -eq (Snapshot $app.Id).Threads } 30000
  $w = Idle-Window $app.Id $IdleSeconds
  [pscustomobject]@{ ColdMs = $cold; HookReadyMs = $hook; Before = $w.A; After = $w.B }
}
# Holds the dictation hotkey $Dictations times, recording how long the overlay took to show and to hide after release; on the first hold it waits for memory to settle and samples Cue alone and Cue with its speech worker. Returns Show and Hide (ms lists), RecordingMB and RecordingTreeMB.
function Measure-Dictations {
  $show = @(); $hide = @(); $rec = $null; $tree = $null
  for ($i = 0; $i -lt $Dictations; $i++) {
    Hold @(,$LCTRL) 0x20 0x39
    $show += Await "the overlay to show after the hotkey" { [Win32]::IsWindowVisible([Win32]::FindWindow("CueOverlay", [NullString]::Value)) } 3000
    if ($i -eq 0) { Start-Sleep -Milliseconds 2000; $rec = (Quiet 10000).PrivateMB; $tree = (Snapshot $p.Id $true).PrivateMB } else { Start-Sleep -Milliseconds 700 }
    $sw = [Diagnostics.Stopwatch]::StartNew(); Release @(,$LCTRL) 0x20 0x39
    $null = Await "the overlay to hide after release" { -not ([Win32]::IsWindowVisible([Win32]::FindWindow("CueOverlay", [NullString]::Value))) } 20000; $hide += $sw.ElapsedMilliseconds
    Start-Sleep -Milliseconds 300
  }
  [pscustomobject]@{ Show = $show; Hide = $hide; RecordingMB = $rec; RecordingTreeMB = $tree }
}

# Writes the start-up rows (medians over the runs) and the recipe into the result. Inputs: the result map, the Measure-Startup runs.
function Add-Startup($result, $runs) {
  $result.cold_start_ms = [int](Percentile ($runs | ForEach-Object ColdMs) 50)
  $result.hook_ready_ms = [int](Percentile ($runs | ForEach-Object HookReadyMs) 50)
  $result.idle_private_bytes = [long](Percentile ($runs | ForEach-Object { $_.After.PrivateBytes }) 50)
  $result.idle_private_mb = [math]::Round($result.idle_private_bytes / 1MB, 2)
  $result.idle_working_set_bytes = [long](Percentile ($runs | ForEach-Object { $_.After.WorkingSetBytes }) 50)
  $result.idle_working_set_mb = [math]::Round($result.idle_working_set_bytes / 1MB, 2)
  $result.idle_threads = [int](Percentile ($runs | ForEach-Object { $_.After.Threads }) 50)
  $result.idle_cpu_seconds = [math]::Round((Percentile ($runs | ForEach-Object { $_.After.Cpu - $_.Before.Cpu }) 50), 3)
  $result.idle_window_seconds = $IdleSeconds
  $result.cycles = $Cycles
  $result.dictations = $Dictations
  $result.commit = $(if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { "local" })
  $result.startup_runs = $Repeat
  if ($Repeat -gt 1) {
    $result.cold_start_runs_ms = @($runs | ForEach-Object ColdMs)
    $result.idle_private_runs_bytes = @($runs | ForEach-Object { $_.After.PrivateBytes })
  }
}

# Runs the dictation stage when a complete speech model is installed: overlay timings, memory while recording, then waits for the speech worker to exit after its unload delay (5 s in the bench settings; a worker that stays throws) and records what is left; then the memory after dictation. Without a model every row is SKIPPED. Inputs: the result map, the model id or $null.
function Add-Dictation($result, $model) {
  if (-not $model) {
    $script:skipped += @("overlay_show_p50_ms", "overlay_show_p95_ms", "overlay_hide_p50_ms", "recording_private_mb", "recording_tree_private_mb", "worker_procs_after_unload", "post_dictation_private_mb", "post_dictation_threads")
    "SKIPPED: dictation stage, no complete speech model is installed in $models (a hotkey press only shows a notice)."
    return
  }
  $d = Measure-Dictations
  $result.overlay_show_p50_ms = Percentile $d.Show 50; $result.overlay_show_p95_ms = Percentile $d.Show 95
  $result.overlay_hide_p50_ms = Percentile $d.Hide 50
  $result.recording_private_mb = $d.RecordingMB; $result.recording_tree_private_mb = $d.RecordingTreeMB
  $null = Await "the speech worker to exit" { (Snapshot $p.Id $true).Procs -eq (Snapshot $p.Id $false).Procs } ((5 + 10) * 1000)
  $result.worker_procs_after_unload = (Snapshot $p.Id $true).Procs - (Snapshot $p.Id $false).Procs
  $after = Quiet 30000
  $result.post_dictation_private_mb = $after.PrivateMB; $result.post_dictation_threads = $after.Threads
}

# Opens settings once cold (its open time and memory), then $Cycles warm open/close rounds: the warm p50, the private memory before and after (the leak), what is left after close and its idle CPU. Input: the result map.
function Add-SettingsLeak($result) {
  $cold = OpenClose $p.Id $Exe
  $result.settings_open_first_ms = $cold.Ms
  $result.settings_open_private_mb = $cold.Stable.PrivateMB; $result.settings_open_procs = $cold.Stable.Procs
  $result.settings_open_private_bytes = $cold.Stable.PrivateBytes; $result.settings_open_peak_private_bytes = $cold.PeakBytes
  $result.settings_open_peak_private_mb = [math]::Round($cold.PeakBytes / 1MB, 2)
  $result.settle_warm_ms = Settle $SettleMs
  $before = Quiet 30000; $open = @()
  for ($i = 0; $i -lt $Cycles; $i++) { $open += (OpenClose $p.Id $Exe).Ms }
  $result.settle_end_ms = Settle $SettleMs
  $after = Quiet 30000
  $result.settings_open_p50_ms = Percentile $open 50
  $result.leak_baseline_mb = $before.PrivateMB; $result.leak_after_mb = $after.PrivateMB
  $result.leak_baseline_bytes = $before.PrivateBytes; $result.leak_after_bytes = $after.PrivateBytes
  $result.leak_delta_mb = [math]::Round($after.PrivateMB - $before.PrivateMB, 2)
  $result.procs_after_close = $after.Procs
  $result.after_close_process_handles = [Handles]::ProcessCount([uint32]$p.Id)
  $result.after_close_private_mb = $after.PrivateMB; $result.after_close_threads = $after.Threads
  $w = Idle-Window $p.Id $IdleSeconds
  $result.after_close_cpu_seconds = [math]::Round($w.B.Cpu - $w.A.Cpu, 3)
}

# Records the size of the exe, of the built frontend's scripts, styles and fonts (SKIPPED without a dist folder) and of the newest installer next to the exe (SKIPPED when none was built). No thresholds. Input: the result map.
function Add-Sizes($result) {
  $result.exe_bytes = (Get-Item $Exe).Length
  $dist = Join-Path $PSScriptRoot "..\..\dist"
  $kinds = [ordered]@{ dist_js_bytes = @("*.js"); dist_css_bytes = @("*.css"); dist_font_bytes = @("*.woff2", "*.woff", "*.ttf") }
  if (Test-Path $dist) { foreach ($k in $kinds.Keys) { $result[$k] = [long](Get-ChildItem $dist -Recurse -File -Include $kinds[$k] | Measure-Object Length -Sum).Sum } }
  else { $script:skipped += @($kinds.Keys); "SKIPPED: frontend sizes, no $dist (build the frontend first)" }
  $setup = Get-ChildItem (Join-Path (Split-Path $Exe) "bundle\nsis") -Filter *-setup.exe -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if ($setup) { $result.installer_bytes = $setup.Length } else { $script:skipped += "installer_bytes"; "SKIPPED: installer_bytes, no installer was built next to the exe" }
}

# Writes the result to $Json and prints every row, then the skipped rows. Input: the result map.
function Write-Result($result) {
  $result | ConvertTo-Json | Set-Content -Path (New-Item -Force -Path $Json).FullName
  $result.GetEnumerator() | ForEach-Object { "{0,-28} {1}" -f $_.Key, $_.Value }
  foreach ($name in $script:skipped) { "SKIPPED: $name" }
}

$p = $null; $script:notepad = $null; $script:notepadFile = $null; $script:skipped = @()
$model = Get-DictationModel
Enter-UserData
try {
  New-Item -Force -ItemType Directory -Path $dir | Out-Null
  if ($Expansions -gt 0) { Copy-Item (Join-Path $PSScriptRoot "..\..\bench\snippets.db") (Join-Path $dir "snippets.db") }
  $speech = $(if ($model) { ",""speech_model"":""$model"",""unload_after_secs"":5" } else { "" })
  Set-Content -Path $settings -Value "{""onboarded"":true,""dictation_mode"":""Hold"",""mic_linger_secs"":1$speech}"
  $result = [ordered]@{ schema = 2 }
  $machine = Get-Machine; foreach ($k in $machine.Keys) { $result[$k] = $machine[$k] }
  $runs = @()
  for ($r = 1; $r -le $Repeat; $r++) {
    $runs += Measure-Startup
    if ($r -lt $Repeat) { Stop-Cue }
  }
  $p = $script:app
  Add-Startup $result $runs
  if ($Expansions -gt 0) { (Measure-ExpansionStage $Expansions).GetEnumerator() | ForEach-Object { $result[$_.Key] = $_.Value } }
  else { $script:skipped += @("snippet_expansion_text_p50_ms", "snippet_expansion_text_p95_ms", "snippet_expansion_total_p50_ms", "snippet_expansion_total_p95_ms", "snippet_include_text_p50_ms", "snippet_include_total_p50_ms") }
  Add-Dictation $result $model
  Add-SettingsLeak $result
  Add-Sizes $result
  Write-Result $result
} catch {
  if ($p) { Dump-Failure "bench failed: $($_.Exception.Message)" $p.Id }
  throw
} finally {
  try { Release @(,$LCTRL) 0x20 0x39; Stop-OwnedNotepad $script:notepad $script:notepadFile }
  finally { Exit-UserData }
}
