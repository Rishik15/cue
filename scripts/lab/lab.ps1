# File: scripts/lab/lab.ps1
# Purpose: maintainer-only experiments (the speech word-error-rate bench, the speech timing bench and the text model benches) behind `scripts/cue.ps1 lab <name>`. They need model files that are not in the repository.
# Contents: Invoke-Cargo, Invoke-Models, Invoke-ModelsBench, Invoke-SpeechBench, Invoke-Script and the name switch.
# Usage: pwsh scripts/lab/lab.ps1 -Name <models|speech|textmodels|texttasks|textsecurity> [-Rest args] [-Scratch dir]   (fixtures live in $env:CUE_LAB, default cue-lab in the user folder)
param([string]$Name, [string[]]$Rest = @(), [string]$Scratch = (Join-Path $env:TEMP "cue"))
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\splat.ps1")
Set-Location (Join-Path $PSScriptRoot "..\..")
if ($env:CUE_ARGS) { $Rest += $env:CUE_ARGS -split "\s+" | Where-Object { $_ } }
$lab = if ($env:CUE_LAB) { $env:CUE_LAB } else { Join-Path $env:USERPROFILE "cue-lab" }
$manifest = "src-tauri\Cargo.toml"

# Runs `cargo test` on the crate with the given words and exits with its code on failure. Input: cargo test arguments.
function Invoke-Cargo([string[]]$words) {
  & cargo test --manifest-path $manifest @words
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# Points CUE_BENCH_SET and CUE_BENCH_MODELS at the spike models in the lab folder unless the caller already set them. Input: the set folder name inside the lab folder.
function Invoke-Models($set) {
  if (-not $env:CUE_BENCH_SET) { $env:CUE_BENCH_SET = Join-Path $lab $set }
  if ($env:CUE_BENCH_MODELS) { return }
  $pairs = "spike-model@Transducer", "spike-v3@Transducer", "spike-canary@Canary", "spike-whisper@Whisper"
  $env:CUE_BENCH_MODELS = ($pairs | ForEach-Object { $dir, $family = $_ -split "@"; "$(Join-Path $lab $dir)@$family" }) -join ";"
}

# Runs the ignored models_bench test (word error rate per model).
function Invoke-ModelsBench {
  Invoke-Models "wer-set"
  Invoke-Cargo "--test", "models_bench", "--", "--ignored", "--nocapture"
}

# Runs the ignored speech_bench test (timings, memory, CPU and word loss; see its header for CUE_BENCH_THREADS, CUE_BENCH_CLIPS, CUE_BENCH_RAW and CUE_BENCH_LABEL).
function Invoke-SpeechBench {
  Invoke-Models "wer-set"
  Invoke-Cargo "--test", "speech_bench", "--", "--ignored", "--nocapture"
}

# Runs a sibling PowerShell script with the extra words as named parameters and exits with its code on failure. Input: script file name.
function Invoke-Script($file) {
  $splat = ConvertTo-Splat $Rest
  & (Join-Path $PSScriptRoot $file) @splat
  if ($LASTEXITCODE) { exit $LASTEXITCODE }
}

switch ($Name) {
  "models" { Invoke-ModelsBench }
  "speech" { Invoke-SpeechBench }
  "textmodels" { Invoke-Script "text-models-bench.ps1" }
  "texttasks" { Invoke-Script "text-tasks-bench.ps1" }
  "textsecurity" { Invoke-Script "text-security-check.ps1" }
  default { Write-Host "Unknown lab '$Name'. Names: models speech textmodels texttasks textsecurity"; exit 2 }
}
