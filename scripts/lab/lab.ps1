# File: scripts/lab/lab.ps1
# Purpose: maintainer-only experiments (the speech word-error-rate bench and the text model benches) behind `scripts/cue.ps1 lab <name>`. They need model files that are not in the repository.
# Contents: Invoke-Cargo, Invoke-ModelsBench, Invoke-Script and the name switch.
# Usage: pwsh scripts/lab/lab.ps1 -Name <models|textmodels|texttasks|textgolden|textlatency|textsecurity> [-Rest args] [-Scratch dir]   (fixtures live in $env:CUE_LAB, default cue-lab in the user folder)
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

# Runs the ignored models_bench test against the spike models in the lab folder (CUE_BENCH_SET and CUE_BENCH_MODELS tell it where they are).
function Invoke-ModelsBench {
  $env:CUE_BENCH_SET = Join-Path $lab "wer-set"
  $pairs = "spike-model@Transducer", "spike-v3@Transducer", "spike-canary@Canary", "spike-whisper@Whisper"
  $env:CUE_BENCH_MODELS = ($pairs | ForEach-Object { $dir, $family = $_ -split "@"; "$(Join-Path $lab $dir)@$family" }) -join ";"
  Invoke-Cargo "--test", "models_bench", "--", "--ignored", "--nocapture"
}

# Runs a sibling PowerShell script with the extra words as named parameters and exits with its code on failure. Input: script file name.
function Invoke-Script($file) {
  $splat = ConvertTo-Splat $Rest
  & (Join-Path $PSScriptRoot $file) @splat
  if ($LASTEXITCODE) { exit $LASTEXITCODE }
}

switch ($Name) {
  "models" { Invoke-ModelsBench }
  "textmodels" { Invoke-Script "text-models-bench.ps1" }
  "texttasks" { Invoke-Script "text-tasks-bench.ps1" }
  "textgolden" { Invoke-Script "text-golden-bench.ps1" }
  "textlatency" { Invoke-Script "text-latency-bench.ps1" }
  "textsecurity" { Invoke-Script "text-security-check.ps1" }
  default { Write-Host "Unknown lab '$Name'. Names: models textmodels texttasks textgolden textlatency textsecurity"; exit 2 }
}
