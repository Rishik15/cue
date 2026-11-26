# File: scripts/test/bench-report.check.ps1
# Purpose: self-check of scripts/verify/bench-report.ps1 on real JSON files in a temp folder: a metric only in the result gets a row, idle CPU is compared as a rate over each side's own window, the leak is n/a across different cycle counts, and a runner result carries the advisory sentence.
# Contents: Get-Report (write both files, run the report, return its text), the cases, the verdict.
# Usage: & scripts/test/bench-report.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$folder = Join-Path $env:TEMP ("cue-bench-report-check-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $folder | Out-Null

# Writes a result and a baseline as JSON, runs the report on them and returns its output as one string. Inputs: the result and baseline hashtables.
function Get-Report($result, $baseline) {
  $r = Join-Path $folder "result.json"; $b = Join-Path $folder "baseline.json"
  ConvertTo-Json $result | Set-Content $r; ConvertTo-Json $baseline | Set-Content $b
  (& (Join-Path $PSScriptRoot "..\verify\bench-report.ps1") -Result $r -Baseline $b) -join "`n"
}

try {
  $base = @{ schema = 2; machine = "PC"; idle_window_seconds = 30; idle_cpu_seconds = 0.3; cycles = 100; leak_delta_mb = 2 }
  $out = Get-Report @{ schema = 2; machine = "PC"; idle_window_seconds = 20; idle_cpu_seconds = 0.2; cycles = 30; leak_delta_mb = 3; settings_open_first_ms = 900 } $base
  Check "a metric only in the result is listed as baseline missing" ($out -match "(?m)^\| settings_open_first_ms \| 900 \|.*\| n/a \(baseline missing\) \|$")
  Check "idle CPU over 20 s and 30 s windows shows equal rates" ($out -match "(?m)^\| idle_cpu_seconds \(per s\) \| 0\.01 \| 0\.01 \| 0% \|$")
  Check "both idle windows are printed" ($out -match "window now 20 s, baseline 30 s")
  Check "the leak across 30 and 100 cycles is n/a" ($out -match "(?m)^\| leak_delta_mb \|.*\| n/a \(cycles 30 vs 100\) \|$")
  Check "a non-runner result has no advisory sentence" ($out -notmatch "advisory")
  $out = Get-Report @{ schema = 2; machine = "github-runner"; idle_window_seconds = 30; idle_cpu_seconds = 0.3; cycles = 100; leak_delta_mb = 3 } $base
  Check "the leak with equal cycles is a percentage" ($out -match "(?m)^\| leak_delta_mb \| 3 \| 2 \| \+50% \|$")
  Check "a runner result carries the advisory sentence" ($out -match "Numbers from a shared runner \(machine: github-runner\) are advisory; compare only with a run on the same machine kind\.")
} finally { Remove-Item -LiteralPath $folder -Recurse -Force }
Complete-Checks
