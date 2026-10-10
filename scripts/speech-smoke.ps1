# Purpose: prove the speech worker lifecycle on Windows: the hotkey starts a worker process, it loads the model, it finishes a job, it is unloaded
# after the idle time, and its memory goes back to the OS. Uses silence from the mic (no audio is played); real speech is the manual check.
# Needs a release build and a folder with the Parakeet v2 files plus silero_vad.onnx (default C:\dev\spike-model; copied into Cue's data dir for the test and removed after).
# Usage: pwsh scripts/speech-smoke.ps1 [-Exe path] [-Model folder]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"),
  [string]$Model = "C:\dev\spike-model"
)
$ErrorActionPreference = "Stop"
Add-Type @'
using System; using System.Runtime.InteropServices;
public class S { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
'@
function Down { [S]::keybd_event(0xA2, 0x1D, 0, [UIntPtr]::Zero); [S]::keybd_event(0x20, 0x39, 0, [UIntPtr]::Zero) }
function Up { [S]::keybd_event(0x20, 0x39, 2, [UIntPtr]::Zero); [S]::keybd_event(0xA2, 0x1D, 2, [UIntPtr]::Zero) }
function Worker { Get-CimInstance Win32_Process -Filter "Name='cue.exe'" | Where-Object { $_.CommandLine -like "*--speech-worker*" } | Select-Object -First 1 }
function PrivateMB($id) { $p = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($p) { [math]::Round($p.PrivateMemorySize64 / 1MB, 1) } else { 0 } }
function WaitUntil($cond, $ms) { $sw = [Diagnostics.Stopwatch]::StartNew(); while (-not (& $cond) -and $sw.ElapsedMilliseconds -lt $ms) { Start-Sleep -Milliseconds 20 }; $sw.ElapsedMilliseconds }

$data = Join-Path $env:APPDATA "dev.cue.app"; $local = Join-Path $env:LOCALAPPDATA "dev.cue.app"
$root = Join-Path $local "models"; $models = Join-Path $root "parakeet-v2"; $vad = Join-Path $root "silero_vad.onnx"; $settings = Join-Path $data "settings.json"; $backup = "$settings.smoke-backup"
$copied = -not ((Test-Path (Join-Path $models "encoder.int8.onnx")) -and (Test-Path $vad))
Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 1
if ($copied) { New-Item -ItemType Directory -Force $models | Out-Null; foreach ($f in "tokens.txt","joiner.int8.onnx","decoder.int8.onnx","encoder.int8.onnx") { Copy-Item (Join-Path $Model $f) $models }; Copy-Item (Join-Path $Model "silero_vad.onnx") $vad }
New-Item -ItemType Directory -Force $data | Out-Null
if (Test-Path $settings) { Copy-Item $settings $backup -Force }
'{"unload_after_secs":5,"dictation_mode":"Hold","min_record_ms":300}' | Set-Content $settings -Encoding ascii
try {
  $err = Join-Path $env:TEMP "cue-speech-smoke.err"; $app = Start-Process $Exe -ArgumentList "--background" -PassThru -RedirectStandardError $err
  Start-Sleep 4
  $idleBefore = PrivateMB $app.Id
  $noWorkerAtIdle = -not (Worker)

  Down
  $loadMs = WaitUntil { Worker } 5000
  $w = Worker
  $started = [bool]$w
  Start-Sleep -Milliseconds 3000; Up   # the microphone takes about a second to open, so hold long enough to record some silence
  $ready = WaitUntil { $w -and (PrivateMB $w.ProcessId) -gt 300 } 20000
  $peak = if ($w) { PrivateMB $w.ProcessId } else { 0 }
  $gone = WaitUntil { -not (Worker) } 25000
  $unloaded = -not (Worker)
  Start-Sleep 3
  $idleAfter = PrivateMB $app.Id

  "no worker at idle: $noWorkerAtIdle | worker started on press: $started (+$loadMs ms) | model loaded after ${ready} ms, worker private $peak MB"
  "unloaded after idle: $unloaded (+$gone ms) | main idle private: $idleBefore MB before, $idleAfter MB after"
  if (Test-Path $err) { Get-Content $err -Tail 12 }
  if ($noWorkerAtIdle -and $started -and $peak -gt 300 -and $unloaded -and $idleAfter -lt 5) { "PASS" } else { "FAIL"; exit 1 }
} finally {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep 1
  if (Test-Path $backup) { Move-Item $backup $settings -Force } else { Remove-Item $settings -ErrorAction SilentlyContinue }
  if ($copied) { Remove-Item $models, $vad -Recurse -Force -ErrorAction SilentlyContinue }
}
