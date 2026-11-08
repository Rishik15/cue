# File: scripts/lab/speech-smoke.ps1
# Purpose: prove the speech worker lifecycle on Windows: the hotkey starts a worker process, it loads the model, it finishes a job, it is unloaded after the idle time, and its memory goes back to the OS. Uses silence from the mic (no audio is played); real speech is the manual check.
# Contents: Down, Up, Worker, PrivateMB, the model copy, the lifecycle steps with state waits, the verdict.
# Needs a release build and a folder with the Parakeet v2 files plus silero_vad.onnx (default spike-model inside $env:CUE_LAB, which is cue-lab in your user folder if unset; copied into Cue's data dir for the test and removed after).
# Usage: pwsh scripts/lab/speech-smoke.ps1 [-Exe path] [-Model folder]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"),
  [string]$Model = (Join-Path $(if ($env:CUE_LAB) { $env:CUE_LAB } else { Join-Path $env:USERPROFILE "cue-lab" }) "spike-model")
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")

# Presses the default dictation shortcut (Ctrl+Space).
function Down { [Win32]::keybd_event(0xA2, 0x1D, 0, [UIntPtr]::Zero); [Win32]::keybd_event(0x20, 0x39, 0, [UIntPtr]::Zero) }
# Releases the default dictation shortcut.
function Up { [Win32]::keybd_event(0x20, 0x39, 2, [UIntPtr]::Zero); [Win32]::keybd_event(0xA2, 0x1D, 2, [UIntPtr]::Zero) }
# Returns the speech worker process (a cue.exe started with --speech-worker), or $null.
function Worker { Get-CimInstance Win32_Process -Filter "Name='cue.exe'" | Where-Object { $_.CommandLine -like "*--speech-worker*" } | Select-Object -First 1 }
# Returns the private memory in MB of a process id, or 0 when it is gone. Input: process id.
function PrivateMB($id) { $p = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($p) { [math]::Round($p.PrivateMemorySize64 / 1MB, 1) } else { 0 } }

$data = Join-Path $env:APPDATA "dev.cue.app"; $local = Join-Path $env:LOCALAPPDATA "dev.cue.app"
$root = Join-Path $local "models"; $models = Join-Path $root "parakeet-v2"; $vad = Join-Path $root "silero_vad.onnx"; $settings = Join-Path $data "settings.json"; $backup = "$settings.smoke-backup"
$copied = -not ((Test-Path (Join-Path $models "encoder.int8.onnx")) -and (Test-Path $vad))
Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
if ($copied) { New-Item -ItemType Directory -Force $models | Out-Null; foreach ($f in "tokens.txt","joiner.int8.onnx","decoder.int8.onnx","encoder.int8.onnx") { Copy-Item (Join-Path $Model $f) $models }; Copy-Item (Join-Path $Model "silero_vad.onnx") $vad }
New-Item -ItemType Directory -Force $data | Out-Null
if (Test-Path $settings) { Copy-Item $settings $backup -Force }
'{"onboarded":true,"unload_after_secs":5,"dictation_mode":"Hold","min_record_ms":300}' | Set-Content $settings -Encoding ascii
try {
  $err = Join-Path $env:TEMP "cue-speech-smoke.err"; $app = Start-Process $Exe -ArgumentList "--background" -PassThru -RedirectStandardError $err
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  $idleBefore = Wait-Stable "the main process memory to settle after start" { PrivateMB $app.Id } 0.5 30000
  $noWorkerAtIdle = -not (Worker)

  Down
  $loadMs = WaitUntil { Worker } 5000
  $w = Worker
  $started = [bool]$w
  Start-Sleep -Milliseconds 3000
  Up
  $ready = WaitUntil { $w -and (PrivateMB $w.ProcessId) -gt 300 } 20000
  $peak = if ($w) { PrivateMB $w.ProcessId } else { 0 }
  $gone = WaitUntil { -not (Worker) } 25000
  $unloaded = -not (Worker)
  $idleAfter = Wait-Stable "the main process memory to settle after the unload" { PrivateMB $app.Id } 0.5 30000

  "no worker at idle: $noWorkerAtIdle | worker started on press: $started (+$loadMs ms) | model loaded after ${ready} ms, worker private $peak MB"
  "unloaded after idle: $unloaded (+$gone ms) | main idle private: $idleBefore MB before, $idleAfter MB after"
  if (Test-Path $err) { Get-Content $err -Tail 12 }
  if ($noWorkerAtIdle -and $started -and $peak -gt 300 -and $unloaded -and $idleAfter -lt 5) { "PASS" } else { "FAIL"; exit 1 }
} finally {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  $null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
  if (Test-Path $backup) { Move-Item $backup $settings -Force } else { Remove-Item $settings -ErrorAction SilentlyContinue }
  if ($copied) { Remove-Item $models, $vad -Recurse -Force -ErrorAction SilentlyContinue }
}
