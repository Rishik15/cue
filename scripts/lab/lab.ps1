# File: scripts/lab/lab.ps1
# Purpose: maintainer-only experiments (speech engine tests against real models, word-error-rate runs, text model benches) behind `scripts/cue.ps1 lab <name>`. They need model files that are not in the repository.
# Usage: pwsh scripts/lab/lab.ps1 -Name <speech|stream|streammulti|streamlong|models|textmodels|texttasks|textsecurity> [-Rest args] [-Scratch dir]   (fixtures live in $env:CUE_LAB, default cue-lab in the user folder)
param([string]$Name, [string[]]$Rest = @(), [string]$Scratch = (Join-Path $env:TEMP "cue"))
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\splat.ps1")
Set-Location (Join-Path $PSScriptRoot "..\..")
if ($env:CUE_ARGS) { $Rest += $env:CUE_ARGS -split "\s+" | Where-Object { $_ } }
$lab = if ($env:CUE_LAB) { $env:CUE_LAB } else { Join-Path $env:USERPROFILE "cue-lab" }
$models = Join-Path $env:LOCALAPPDATA "dev.cue.app\models"
$testModel = Join-Path $Scratch "test-model"
$manifest = "src-tauri\Cargo.toml"

function Invoke-Cargo([string[]]$words) {
  & cargo test --manifest-path $manifest @words
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

function Initialize-TestModel {
  New-Item -ItemType Directory -Force $testModel | Out-Null
  Copy-Item -Recurse -Force (Join-Path $models "parakeet-v2\*") $testModel
  Copy-Item -Force (Join-Path $models "silero_vad.onnx") $testModel
}

function Invoke-Speech {
  $wav = Join-Path $Scratch "test.wav"
  & (Join-Path $PSScriptRoot "make-test-wav.ps1") -Out $wav
  Initialize-TestModel
  $env:CUE_TEST_WAV = $wav
  $env:CUE_TEST_MODEL = $testModel
  $env:CUE_TEST_FAMILY = "Transducer"
  Invoke-Cargo "--test", "worker", "--", "--nocapture"
}

function Invoke-Stream {
  $env:CUE_TEST_OFFLINE_MODEL = $testModel
  $env:CUE_TEST_STREAM_MODEL = Join-Path $models "nemotron-en"
  Invoke-Cargo "--lib", "hypothesis"
  Invoke-Cargo "--test", "stream_worker", "--", "--nocapture"
}

function Invoke-ModelsBench {
  $env:CUE_BENCH_SET = Join-Path $lab "wer-set"
  $pairs = "spike-model@Transducer", "spike-v3@Transducer", "spike-canary@Canary", "spike-whisper@Whisper"
  $env:CUE_BENCH_MODELS = ($pairs | ForEach-Object { $dir, $family = $_ -split "@"; "$(Join-Path $lab $dir)@$family" }) -join ";"
  Invoke-Cargo "--test", "models_bench", "--", "--ignored", "--nocapture"
}

function Invoke-Script($file) {
  $splat = ConvertTo-Splat $Rest
  & (Join-Path $PSScriptRoot $file) @splat
  if ($LASTEXITCODE) { exit $LASTEXITCODE }
}

switch ($Name) {
  "speech" { Invoke-Speech }
  "stream" { Invoke-Stream }
  "streammulti" {
    $env:CUE_TEST_STREAM_MODEL = Join-Path $models "nemotron-multi"
    $env:CUE_TEST_STREAM_FAMILY = "StreamingMulti"
    $env:CUE_TEST_STREAM_WAVS = "ar.wav,uk.wav,vi.wav"
    Invoke-Cargo "--test", "stream_worker", "streaming_worker", "--", "--nocapture"
  }
  "streamlong" {
    $env:CUE_TEST_STREAM_MODEL = Join-Path $models "nemotron-en"
    Invoke-Cargo "--test", "stream_worker", "--", "--ignored", "--nocapture"
  }
  "models" { Invoke-ModelsBench }
  "textmodels" { Invoke-Script "text-models-bench.ps1" }
  "texttasks" { Invoke-Script "text-tasks-bench.ps1" }
  "textsecurity" { Invoke-Script "text-security-check.ps1" }
  default { Write-Host "Unknown lab '$Name'. Names: speech stream streammulti streamlong models textmodels texttasks textsecurity"; exit 2 }
}
