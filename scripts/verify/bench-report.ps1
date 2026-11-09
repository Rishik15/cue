# File: scripts/verify/bench-report.ps1
# Purpose: show a bench result next to bench/baseline.json as a Markdown table, without judging it. Performance is tracked, not gated: a person reads the table.
# Contents: Format-Change (one change cell), the table, the machine note, the optional step-summary output.
# Usage: pwsh scripts/verify/bench-report.ps1 [-Result bench/latest.json] [-Baseline bench/baseline.json] [-Summary path]   (-Summary appends the table to a file, for $env:GITHUB_STEP_SUMMARY)
param(
  [string]$Result = (Join-Path $PSScriptRoot "..\..\bench\latest.json"),
  [string]$Baseline = (Join-Path $PSScriptRoot "..\..\bench\baseline.json"),
  [string]$Summary = ""
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Result)) { throw "No result to report: $Result. Run the bench first." }
$now = Get-Content $Result -Raw | ConvertFrom-Json
$base = Get-Content $Baseline -Raw | ConvertFrom-Json
$info = "schema", "machine", "os_build", "webview2", "cores", "idle_window_seconds"

# Returns the change cell for one metric: a percentage, or n/a plus the plain difference when the baseline is zero, missing or from another schema. Inputs: current value, baseline value.
function Format-Change($cur, $ref) {
  if ($null -eq $cur) { return "skipped" }
  if ($null -eq $ref -or $ref -eq 0) { return "n/a (baseline $(if ($null -eq $ref) { 'missing' } else { '0' }))" }
  "{0:+0.#;-0.#;0}%" -f (($cur - $ref) / $ref * 100)
}

$rows = @("| metric | now | baseline | change |", "| --- | ---: | ---: | ---: |")
foreach ($m in $base.PSObject.Properties | Where-Object { $info -notcontains $_.Name }) {
  $cur = $now.($m.Name)
  $rows += "| $($m.Name) | $(if ($null -eq $cur) { 'skipped' } else { $cur }) | $($m.Value) | $(Format-Change $cur $m.Value) |"
}
$note = "Measured on $($now.machine), Windows $($now.os_build), WebView2 $($now.webview2), $($now.cores) cores. Baseline from $($base.machine), Windows $($base.os_build). A table from a different kind of machine shows direction only."
if ($now.schema -ne $base.schema) { $note += " Schemas differ ($($now.schema) vs $($base.schema)): compare by eye." }
$text = ($rows + "" + $note) -join "`n"
$text
if ($Summary) { Add-Content -Path $Summary -Value "### Bench against bench/baseline.json`n`n$text" }
